//! PDF text parsing for village records.

use std::path::Path;

/// A parsed village record from the Kemendagri PDF.
#[derive(serde::Serialize, Clone)]
pub(crate) struct VillageRecord {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) district: String,
    pub(crate) city: String,
    pub(crate) province: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) raw_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) note_keyword: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) note_boundary: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) district_note: Option<String>,
}

/// How much detail to include when saving parsed village records to JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseOutputDetail {
    /// Code and cleaned name only (same as what goes into the database).
    Minimal,
    /// Code, cleaned name, and raw name before note stripping / truncation.
    WithRawName,
    /// Code, cleaned name, raw name, and note detection metadata
    /// (keyword matched, boundary position).
    Full,
}

/// Result of extracting a village name from PDF text, with optional metadata.
pub(crate) struct ExtractedName {
    /// The cleaned village name (after note stripping + 5-word truncation).
    pub(crate) name: String,
    /// The raw text before note stripping and truncation (if different from name).
    pub(crate) raw_name: Option<String>,
    /// The note keyword that was detected, if any.
    pub(crate) note_keyword: Option<String>,
    /// The byte position where the note boundary was found.
    pub(crate) note_boundary: Option<usize>,
}

/// Result of extracting a district name from a kecamatan line in PDF text.
pub(crate) struct ExtractedDistrict {
    /// The cleaned district name (after column splitting + note stripping).
    pub(crate) name: String,
    /// Annotation text from the kecamatan line (e.g., "Semula wil. Provinsi Papua Barat; UU No. 16 Tahun 2025").
    pub(crate) note: Option<String>,
}

/// A parsed section header from the PDF (province + city grouping).
pub(crate) struct SectionHeader<'a> {
    pub(crate) province: &'a str,
    pub(crate) city: &'a str,
}

/// Note keywords that indicate administrative annotation text following a village name.
///
/// These are split into two categories:
/// - **Self-validating**: always indicate a note, regardless of what follows.
///   E.g., "Semula" (formerly) always means the rest is a note about the former status.
/// - **Reference-validated**: only indicate a note if followed by a reference-like pattern.
///   E.g., "UU" could appear in a village name like "UU Jaya", but "UU No. 4/2002" is a note.
const SELF_VALIDATING_KEYWORDS: &[&str] = &[
    "Semula",
    "Semual",
    "Semuila",
    "Smula",
    "Menjadi",
    "Berubah",
    "Penataan",
    "Pengkatan",
    "Penghapusan",
    "Lampiran",
    "Letak",
    "PMD",
];

const REFERENCE_VALIDATED_KEYWORDS: &[&str] = &[
    "Perbaikan",
    "Pemekaran",
    "Qonun",
    "Qanun",
    "Koreksi",
    "Penggabungan",
    "Pembentukan",
    "Penetapan",
    "Perubahan",
    "Peningkatan",
    "Pemecahan",
    "Amar",
    "Perda",
    "Perbup",
    "Kepbup",
    "Berdasarkan",
    "PP",
    "UU",
    "ND",
    "Surat",
    "Srt",
    "Ds.",
    "Afd.",
    "wil. Kec",
    "wil Kec",
    "Nagari hasil",
    "Hal Hasil",
];

/// Patterns that confirm a keyword match is followed by a reference (not part of a name).
const REFERENCE_INDICATORS: &[&str] = &[
    "no.",
    "nomor",
    "nama",
    "wil",
    "menjadi",
    "sebagai",
    "desa",
    "gampong",
    "nagari",
    "kec.",
    "kec ",
    "dari",
    "perda",
    "perbup",
    "kepbup",
    "qanun",
    "qonun",
    "uu",
    "pmd",
    "pp",
    "pemekaran",
    "perbaikan",
    "penggabungan",
    "pembentukan",
    "penetapan",
    "perubahan",
    "koreksi",
    "amar",
    "putusan",
    "surat",
    "status",
    "hasil",
    "sebagian",
    "berdasarkan",
];

/// Maximum number of words in an extracted village name.
const MAX_NAME_WORDS: usize = 5;

/// Check whether the text following a keyword contains a reference-like pattern.
///
/// Scans up to `window` bytes after `pos` for any indicator that confirms
/// this is an administrative note rather than part of the village name.
fn has_reference_indicator(text_lower: &str, pos: usize, window: usize) -> bool {
    let start = pos;
    let end = (pos + window).min(text_lower.len());
    if start >= end {
        return false;
    }
    let window_text = &text_lower[start..end];
    if window_text.chars().any(|c| c.is_ascii_digit()) {
        return true;
    }
    for indicator in REFERENCE_INDICATORS {
        if window_text.contains(indicator) {
            return true;
        }
    }
    false
}

/// Check whether the keyword match at `pos..pos + kw_len` in `text` is at a word boundary.
///
/// A word boundary means the character before the match start and the character
/// after the match end are not alphanumeric (or the start/end of the string).
/// This prevents keywords like "ND" from matching inside words like "poNDok".
fn is_word_boundary(text: &str, pos: usize, kw_len: usize) -> bool {
    let bytes = text.as_bytes();
    let before_ok = pos == 0 || !bytes[pos - 1].is_ascii_alphanumeric();
    let after_ok = pos + kw_len >= text.len() || !bytes[pos + kw_len].is_ascii_alphanumeric();
    before_ok && after_ok
}

/// Result of searching for a note keyword in village name text.
struct NoteMatch {
    /// Byte position where the note keyword starts.
    pos: usize,
    /// The keyword that was matched (original casing).
    keyword: &'static str,
}

/// Find the earliest note boundary in `raw` by checking all note keywords.
///
/// Self-validating keywords always mark a note boundary.
/// Reference-validated keywords only mark a boundary if followed by a
/// reference-like pattern within 30 characters.
///
/// Returns the earliest match if found, or `None` if no note keyword was detected.
fn find_note_boundary(raw_lower: &str) -> Option<NoteMatch> {
    let mut best: Option<NoteMatch> = None;

    for keyword in SELF_VALIDATING_KEYWORDS {
        let kw_lower = keyword.to_lowercase();
        if let Some(pos) = raw_lower.find(&kw_lower) {
            if is_word_boundary(raw_lower, pos, kw_lower.len())
                && best.as_ref().is_none_or(|b| pos < b.pos)
            {
                best = Some(NoteMatch { pos, keyword });
            }
        }
    }

    for keyword in REFERENCE_VALIDATED_KEYWORDS {
        let kw_lower = keyword.to_lowercase();
        if let Some(pos) = raw_lower.find(&kw_lower) {
            if is_word_boundary(raw_lower, pos, kw_lower.len())
                && has_reference_indicator(raw_lower, pos + kw_lower.len(), 30)
                && best.as_ref().is_none_or(|b| pos < b.pos)
            {
                best = Some(NoteMatch { pos, keyword });
            }
        }
    }

    best
}

/// Pre-compiled regex patterns for parsing village records from PDF text.
///
/// Compiling all four regex patterns once avoids re-compilation per call.
pub(crate) struct VillageParser {
    village_code_re: regex::Regex,
    kecamatan_code_re: regex::Regex,
    name_re: regex::Regex,
    section_header_re: regex::Regex,
}

impl VillageParser {
    /// Create a new parser with pre-compiled regex patterns.
    pub(crate) fn new() -> Self {
        Self {
            village_code_re: regex::Regex::new(r"^(\d{2}\.\d{2}\.\d{2}\.\d{4})(?:\s|$)").unwrap(),
            kecamatan_code_re: regex::Regex::new(r"^\s*(\d{2}\.\d{2}\.\d{2})\s+\d+\s+([A-Z0-9])")
                .unwrap(),
            name_re: regex::Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap(),
            section_header_re: regex::Regex::new(r"C\.\w+\.\d+\)\s+(.+)$").unwrap(),
        }
    }

    /// Parse village records from extracted PDF text.
    ///
    /// Returns a list of `VillageRecord` with code, name, district, city, and province.
    pub(crate) fn parse(&self, text: &str) -> Vec<VillageRecord> {
        eprintln!("Parsing village records...");

        let mut villages = Vec::new();
        let mut current_province = "";
        let mut current_city = "";
        let mut current_district_code = String::new();
        let mut current_district_name = String::new();
        let mut current_district_note: Option<String> = None;

        for line in text.lines() {
            if let Some(header) = parse_section_header(line, &self.section_header_re) {
                current_province = header.province;
                current_city = header.city;
                current_district_code.clear();
                current_district_name.clear();
                current_district_note = None;
            }

            if let Some(cap) = self.kecamatan_code_re.captures(line) {
                current_district_code = cap.get(1).unwrap().as_str().to_string();
                let after_prefix = &line[cap.get(0).unwrap().start()..];
                let extracted = extract_district_name(after_prefix);
                current_district_name = extracted.name;
                current_district_note = extracted.note;
                continue;
            }

            if let Some(code) = self.village_code_re.captures(line).and_then(|c| c.get(1)) {
                let code_str = code.as_str().to_string();
                current_district_code = code_str[..8].to_string();

                let after_code = &line[code.end()..];
                if let Some(extracted) = extract_village_name(after_code, &self.name_re) {
                    villages.push(VillageRecord {
                        code: code_str,
                        name: extracted.name,
                        district: if current_district_name.is_empty() {
                            current_district_code.clone()
                        } else {
                            current_district_name.clone()
                        },
                        city: current_city.to_string(),
                        province: current_province.to_string(),
                        raw_name: extracted.raw_name,
                        note_keyword: extracted.note_keyword,
                        note_boundary: extracted.note_boundary,
                        district_note: current_district_note.clone(),
                    });
                }
            }
        }

        eprintln!("Parsed {} villages", villages.len());
        villages
    }
}

/// Parse village records from extracted PDF text.
///
/// Convenience wrapper around [`VillageParser::parse`].
pub(crate) fn parse_villages(text: &str) -> Vec<VillageRecord> {
    VillageParser::new().parse(text)
}

/// Note keywords to look for in district name suffixes (after column splitting).
///
/// These are a subset of village note keywords — only ones that commonly appear
/// in kecamatan annotation columns.
const DISTRICT_NOTE_KEYWORDS: &[&str] = &[
    "Semula",
    "Semual",
    "Semuila",
    "Smula",
    "Menjadi",
    "Perbaikan",
    "Pemekaran",
    "Koreksi",
    "Perda",
    "Perbup",
    "Kepbup",
    "Berdasarkan",
    "PP",
    "UU",
    "Qanun",
    "Berubah",
    "Penataan",
    "Penghapusan",
    "Penggabungan",
    "Pembentukan",
];

/// Minimum consecutive spaces that indicate a column boundary in pdftotext -layout output.
const COLUMN_GAP_SPACES: usize = 3;

/// Find the byte position where the first column gap (3+ consecutive spaces) starts.
///
/// Returns `Some(pos)` where `pos` is the byte offset of the first space in the
/// first 3+ consecutive space run. Returns `None` if the text has no column gap.
///
/// This is used in `extract_village_name` to detect Format 2 PDF lines where the
/// village name and annotation text are separated by a wide space gap.
fn first_gap_position(raw: &str) -> Option<usize> {
    let bytes = raw.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b' ' {
            let start = i;
            while i < len && bytes[i] == b' ' {
                i += 1;
            }
            if i - start >= COLUMN_GAP_SPACES {
                return Some(start);
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Split text on 3+ consecutive spaces (the column separator in `pdftotext -layout` output).
///
/// Returns non-empty trimmed parts. For example:
/// `"Bakongan                               7"` → `["Bakongan", "7"]`
/// `"Abenaho                   Semula wil Prov."` → `["Abenaho", "Semula wil Prov."]`
fn split_on_column_gap(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_gap = false;
    let mut gap_start = 0;
    let mut space_count = 0;

    for (i, c) in text.char_indices() {
        if c == ' ' {
            if !in_gap {
                in_gap = true;
                gap_start = i;
            }
            space_count += 1;
        } else {
            if in_gap && space_count >= COLUMN_GAP_SPACES {
                let part = text[start..gap_start].trim();
                if !part.is_empty() {
                    parts.push(part);
                }
                start = i;
            }
            in_gap = false;
            space_count = 0;
        }
    }

    let last = text[start..].trim();
    if !last.is_empty() {
        parts.push(last);
    }

    parts
}

/// Check if a string matches the `XX.XX.XX` kecamatan code pattern.
fn is_kecamatan_code(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 8
        && bytes[2] == b'.'
        && bytes[5] == b'.'
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[3].is_ascii_digit()
        && bytes[4].is_ascii_digit()
        && bytes[6].is_ascii_digit()
        && bytes[7].is_ascii_digit()
}

/// Extract the district name and optional annotation note from a kecamatan line.
///
/// Kecamatan lines in `pdftotext -layout` output have two formats:
///
/// **Format 1** (table): `CODE NUMBER NAME [columns...] [note]`
/// ```text
/// 11.01.01 1 Bakongan                               7
/// 11.01.10 10 Pasie Raja  21  Perbaikan nama...
/// 96.01.01 1 Makbon  1  14  Semula wil. Provinsi...
/// ```
///
/// **Format 2** (listing): `CODE  NUMBER NAME [note]`
/// ```text
/// 96.01.01                    1 Makbon    Semula wil. Provinsi...
/// ```
///
/// The extraction flow:
/// 1. `skip_code_prefix` — skip past `CODE NUMBER ` to reach the name portion
/// 2. Split on 3+ consecutive spaces (column separator) → `[name_part, rest...]`
/// 3. If `name_part` matches `XX.XX.XX` code pattern, take the next part as name (Format 2)
/// 4. Apply note keyword stripping to the district name
/// 5. Extract annotation note from remaining suffix if it contains note keywords
fn extract_district_name(after_prefix: &str) -> ExtractedDistrict {
    let trimmed = after_prefix.trim();
    let name_and_rest = skip_code_prefix(trimmed);

    let parts = split_on_column_gap(name_and_rest);

    let (raw_name, suffix_start) = if !parts.is_empty() && is_kecamatan_code(parts[0]) {
        if parts.len() > 1 {
            (parts[1], 2)
        } else {
            (parts[0], parts.len())
        }
    } else {
        (parts.first().copied().unwrap_or(""), 1)
    };

    let stripped = strip_trailing_count(raw_name);

    let (cleaned_name, name_note) = strip_district_note(stripped);

    let suffix_parts: Vec<&str> = parts[suffix_start.min(parts.len())..].to_vec();
    let suffix_note = if !suffix_parts.is_empty() {
        extract_suffix_note(suffix_parts.join(" "))
    } else {
        None
    };

    let note = name_note.or(suffix_note);

    ExtractedDistrict {
        name: cleaned_name.to_string(),
        note,
    }
}

/// Apply note keyword stripping to a district name (for cases where the note keyword
/// is embedded in the name column before a column gap, e.g., "Abenaho Semula wil Prov.").
fn strip_district_note(name: &str) -> (&str, Option<String>) {
    let name_lower = name.to_lowercase();
    for keyword in DISTRICT_NOTE_KEYWORDS {
        let kw_lower = keyword.to_lowercase();
        if let Some(pos) = name_lower.find(&kw_lower) {
            let cleaned = name[..pos].trim();
            if !cleaned.is_empty() {
                let note_text = name[pos..].trim().to_string();
                return (cleaned, Some(note_text));
            }
        }
    }
    (name, None)
}

/// Extract an annotation note from the suffix after the district name column.
///
/// The suffix may contain counts (like "7" or "- 7" or "1  14") before the
/// actual note text. This function finds the earliest note keyword in the
/// suffix and returns the text from that point onward.
fn extract_suffix_note(suffix: String) -> Option<String> {
    let suffix_lower = suffix.to_lowercase();
    let earliest = DISTRICT_NOTE_KEYWORDS
        .iter()
        .filter_map(|kw| {
            let kw_lower = kw.to_lowercase();
            suffix_lower.find(&kw_lower).map(|pos| (pos, kw))
        })
        .min_by_key(|(pos, _)| *pos);

    earliest.map(|(pos, _)| suffix[pos..].trim().to_string())
}

/// Skip the code and number prefix in a kecamatan line to get to the name.
///
/// Input like `"31.73.01 60 KECAMATAN BALEENDAH"` — the prefix is
/// `CODE SPACE NUMBER SPACE`. We skip past the code (d+d.d+d.d+d pattern),
/// then the number, to reach the district name.
///
/// For names starting with a digit (e.g., "2 x 11 Anam Lingkuang 3"),
/// we need to skip only the code prefix and the first number, then take
/// the rest including any leading digits in the name.
fn skip_code_prefix(s: &str) -> &str {
    let mut chars = s.char_indices().peekable();
    let mut dot_count = 0;
    let mut past_code = false;
    let mut past_number = false;
    let mut name_start = 0;

    while let Some((idx, c)) = chars.next() {
        if !past_code {
            if c == '.' {
                dot_count += 1;
            } else if dot_count >= 2 && c.is_ascii_whitespace() {
                past_code = true;
            }
        } else if !past_number {
            if c.is_ascii_digit() {
                let all_digits = chars.peek().is_none_or(|&(_, nc)| !nc.is_ascii_digit());
                if all_digits {
                    past_number = true;
                }
            } else if c.is_ascii_whitespace() {
                continue;
            } else {
                break;
            }
        } else if c.is_ascii_whitespace() {
            continue;
        } else {
            name_start = idx;
            break;
        }
    }

    if name_start > 0 {
        s[name_start..].trim()
    } else {
        s
    }
}

/// Strip trailing count patterns from a district name.
///
/// Handles patterns like `" - 7"`, `" -7"`, `" 7"` when the trailing portion
/// looks like a count (1-3 digits) rather than part of the name.
///
/// A digit sequence is only stripped if it is preceded by a separator pattern
/// (space + optional dash/spaces) AND the digit sequence has a word boundary
/// before it (i.e., preceded by a space or dash, not a letter/digit).
/// Additionally the stripped digit count must be 1-3 digits (typical village count range).
fn strip_trailing_count(name: &str) -> &str {
    let trimmed = name.trim_end();
    if trimmed.is_empty() {
        return trimmed;
    }

    let bytes = trimmed.as_bytes();
    let len = bytes.len();

    let mut digit_end = len;
    while digit_end > 0 && bytes[digit_end - 1].is_ascii_digit() {
        digit_end -= 1;
    }

    let digit_count = len - digit_end;
    if digit_count == 0 || digit_count > 3 {
        return trimmed;
    }

    if digit_end > 0 && bytes[digit_end - 1] == b'-' {
        let before = trimmed[..digit_end - 1].trim_end();
        if !before.is_empty() {
            return before;
        }
    }

    let before_space = trimmed[..digit_end].trim_end_matches(' ');
    if before_space.len() < digit_end && !before_space.is_empty() {
        let bs_bytes = before_space.as_bytes();
        if bs_bytes[bs_bytes.len() - 1] == b'-' {
            let before_dash = before_space[..bs_bytes.len() - 1].trim_end();
            if !before_dash.is_empty() {
                return before_dash;
            }
        }
    }

    if !before_space.is_empty()
        && !before_space
            .split_whitespace()
            .any(|word| word.chars().any(|c| c.is_ascii_digit()))
    {
        return before_space;
    }

    trimmed
}

/// Extract a village name from the text after the village code, stripping notes.
///
/// For Format 2 PDF lines (column-gap layout), the village name occupies the first
/// column and annotation text appears in subsequent columns separated by 3+ spaces.
/// Column-gap splitting isolates the name, fixing cases where undetected note keywords
/// (OCR typos, abbreviations, "Nagari", "Surat", etc.) bleed into the village name.
///
/// The algorithm:
/// 1. Detect column gap (3+ consecutive spaces) in the raw captured text
/// 2. Find note keyword boundary via `find_note_boundary` on the full raw text
/// 3. Cut at the earlier of: column-gap position, keyword-boundary position
///    - If keyword is before the gap: keyword wins (note prefix in name column)
///    - If gap is before the keyword: gap wins (undetected annotation after gap)
/// 4. Truncate to `MAX_NAME_WORDS` words
pub(crate) fn extract_village_name(
    after_code: &str,
    name_re: &regex::Regex,
) -> Option<ExtractedName> {
    let cap = name_re.captures(after_code)?;
    let raw = cap.get(1)?.as_str().trim();
    if raw.is_empty() || raw.chars().next().map(|c| c.is_numeric()).unwrap_or(false) {
        return None;
    }

    let raw_lower = raw.to_lowercase();
    let note_match = find_note_boundary(&raw_lower);
    let gap_pos = first_gap_position(raw);

    let cleaned = if let Some(gp) = gap_pos {
        let keyword_pos = note_match.as_ref().map_or(usize::MAX, |n| n.pos);
        let cut = gp.min(keyword_pos);
        let c = raw[..cut].trim();
        if c.is_empty() {
            return None;
        }
        c
    } else {
        match &note_match {
            Some(note) => {
                let c = raw[..note.pos].trim();
                if c.is_empty() {
                    return None;
                }
                c
            }
            None => raw,
        }
    };

    let truncated: String = cleaned
        .split_whitespace()
        .take(MAX_NAME_WORDS)
        .collect::<Vec<_>>()
        .join(" ");

    if truncated.is_empty() {
        return None;
    }

    let raw_name = if truncated != raw {
        Some(raw.to_string())
    } else {
        None
    };

    let (note_keyword, note_boundary) = match note_match {
        Some(note) => (Some(note.keyword.to_string()), Some(note.pos)),
        None => (None, None),
    };

    Some(ExtractedName {
        name: truncated,
        raw_name,
        note_keyword,
        note_boundary,
    })
}

/// Parse a section header line (e.g., `C.Kabupaten.1) Kabupaten Bogor Provinsi Jawa Barat`).
pub(crate) fn parse_section_header<'a>(
    line: &'a str,
    re: &regex::Regex,
) -> Option<SectionHeader<'a>> {
    if let Some(cap) = re.captures(line) {
        let text = cap.get(1)?.as_str();
        if let Some(prov_idx) = text.find("Provinsi ") {
            let city = text[..prov_idx].trim();
            let province = text[prov_idx..].trim();
            Some(SectionHeader { province, city })
        } else {
            None
        }
    } else {
        None
    }
}

/// Save parsed village records to a JSON file.
///
/// The level of detail is controlled by `detail`:
/// - `Minimal`: code + cleaned name + district + city + province
/// - `WithRawName`: adds `raw_name` field (original text before note stripping)
/// - `Full`: adds `note_keyword` and `note_boundary` fields
pub(crate) fn save_parsed_villages(
    villages: &[VillageRecord],
    detail: ParseOutputDetail,
    path: &Path,
) -> Result<(), super::PipelineError> {
    use super::PipelineResultExt;
    let output: Vec<VillageRecord> = villages
        .iter()
        .map(|v| match detail {
            ParseOutputDetail::Minimal => VillageRecord {
                code: v.code.clone(),
                name: v.name.clone(),
                district: v.district.clone(),
                city: v.city.clone(),
                province: v.province.clone(),
                raw_name: None,
                note_keyword: None,
                note_boundary: None,
                district_note: None,
            },
            ParseOutputDetail::WithRawName => VillageRecord {
                code: v.code.clone(),
                name: v.name.clone(),
                district: v.district.clone(),
                city: v.city.clone(),
                province: v.province.clone(),
                raw_name: v.raw_name.clone(),
                note_keyword: None,
                note_boundary: None,
                district_note: v.district_note.clone(),
            },
            ParseOutputDetail::Full => v.clone(),
        })
        .collect();

    let json_str =
        serde_json::to_string_pretty(&output).ctx("failed to serialize parsed villages")?;
    std::fs::write(path, json_str).ctx("failed to write parsed villages JSON")?;
    eprintln!("Saved {} parsed villages to {:?}", villages.len(), path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use std::sync::OnceLock;

    fn name_re() -> &'static Regex {
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap())
    }

    fn section_header_re() -> &'static Regex {
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new(r"C\.\w+\.\d+\)\s+(.+)$").unwrap())
    }

    #[test]
    fn test_parse_section_header_with_city_and_province() {
        let line = "C.Kabupaten.1) Kabupaten Bogor Provinsi Jawa Barat";
        let header = parse_section_header(line, section_header_re());
        assert!(header.is_some());
        let h = header.unwrap();
        assert_eq!(h.province, "Provinsi Jawa Barat");
        assert_eq!(h.city, "Kabupaten Bogor");
    }

    #[test]
    fn test_parse_section_header_province_only() {
        let line = "C.Provinsi.1) Provinsi DKI Jakarta";
        let header = parse_section_header(line, section_header_re());
        assert!(header.is_some());
        let h = header.unwrap();
        assert_eq!(h.province, "Provinsi DKI Jakarta");
        assert_eq!(h.city, "");
    }

    #[test]
    fn test_parse_section_header_no_provinsi() {
        let line = "C.Kabupaten.1) Some text without Provinsi";
        assert!(parse_section_header(line, section_header_re()).is_none());
    }

    #[test]
    fn test_parse_section_header_no_match() {
        let line = "31.12.24.2002 ABADMULIA KEC. BUKIT SARI";
        assert!(parse_section_header(line, section_header_re()).is_none());
    }

    #[test]
    fn test_parse_section_header_rejects_invalid_format() {
        let line = "C.Kabupaten.X) Kabupaten Bogor Provinsi Jawa Barat";
        assert!(parse_section_header(line, section_header_re()).is_none());
    }

    #[test]
    fn test_extract_village_name_basic() {
        let name_re = name_re();
        let after_code = " 12 ABADIJAYA";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"ABADIJAYA".to_string())
        );
    }

    #[test]
    fn test_extract_village_name_multi_word() {
        let name_re = name_re();
        let after_code = " 12 SUKA MAJU";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"SUKA MAJU".to_string())
        );
    }

    #[test]
    fn test_extract_village_name_keyword_stripping() {
        let name_re = name_re();
        let after_code = " 15 SUKAMAJU KEMENANGAN Pemekaran menjadi SUKAMAJU";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"SUKAMAJU KEMENANGAN".to_string())
        );
    }

    #[test]
    fn test_extract_village_name_numeric_start() {
        let name_re = name_re();
        let after_code = " 20 5SAFARI Some text";
        let name = extract_village_name(after_code, name_re);
        assert!(name.is_none());
    }

    #[test]
    fn test_extract_village_name_empty() {
        let name_re = name_re();
        let after_code = " 30 ";
        let name = extract_village_name(after_code, name_re);
        assert!(name.is_none());
    }

    #[test]
    fn test_extract_village_name_truncate_to_five_words() {
        let name_re = name_re();
        let after_code = " 10 DESA SUKAMAJU KECAMATAN BUKIT SARI LAINNYA";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"DESA SUKAMAJU KECAMATAN BUKIT SARI".to_string())
        );
    }

    #[test]
    fn test_extract_village_name_six_words_truncated() {
        let name_re = name_re();
        let after_code = " 10 A B C D E F";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"A B C D E".to_string())
        );
    }

    #[test]
    fn test_extract_village_name_semula() {
        let name_re = name_re();
        let after_code = " 2 RAMBONG Semula wil Kec. Bakongan Perda No. 3/2010";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(name.as_ref().map(|e| &e.name), Some(&"RAMBONG".to_string()));
    }

    #[test]
    fn test_extract_village_name_qanun() {
        let name_re = name_re();
        let after_code = " 23 PIRAK TIMU Qanun No. 32/2005";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"PIRAK TIMU".to_string())
        );
    }

    #[test]
    fn test_extract_village_name_uu_with_reference() {
        let name_re = name_re();
        let after_code = " 5 SUKAMAKMUR UU No. 4/2002";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"SUKAMAKMUR".to_string())
        );
    }

    #[test]
    fn test_extract_village_name_uu_without_reference_not_stripped() {
        let name_re = name_re();
        let after_code = " 5 UU JAYA";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(name.as_ref().map(|e| &e.name), Some(&"UU JAYA".to_string()));
    }

    #[test]
    fn test_extract_village_name_hasil_in_name_not_stripped() {
        let name_re = name_re();
        let after_code = " 18 HASIL JAYA";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"HASIL JAYA".to_string())
        );
    }

    #[test]
    fn test_extract_village_name_hal_hasil_with_reference() {
        let name_re = name_re();
        let after_code = " 18 LIYA BAHARI Hal Hasil Klarifikasi Nama Desa";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"LIYA BAHARI".to_string())
        );
    }

    #[test]
    fn test_extract_village_name_amar_with_reference() {
        let name_re = name_re();
        let after_code = " 12 MERDEKA Amar Putusan Mahkamah Agung RI Nomor 395K";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(name.as_ref().map(|e| &e.name), Some(&"MERDEKA".to_string()));
    }

    #[test]
    fn test_extract_village_name_perda_with_number() {
        let name_re = name_re();
        let after_code = " 9 LEUBOK PASI Perda No. 3/2010 tentang pemekaran";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"LEUBOK PASI".to_string())
        );
    }

    #[test]
    fn test_extract_village_name_five_word_name_preserved() {
        let name_re = name_re();
        let after_code = " 7 TANAH SIRAH PIAI NAN XX";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"TANAH SIRAH PIAI NAN XX".to_string())
        );
    }

    #[test]
    fn test_extract_village_name_perbaikan_with_nama() {
        let name_re = name_re();
        let after_code = " 2 UJONG MANGKI Perbaikan nama sesuai Surat Pemkab";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"UJONG MANGKI".to_string())
        );
    }

    #[test]
    fn test_extract_village_name_nd_with_reference() {
        let name_re = name_re();
        let after_code = " 5 SUKAJADI ND Rekom No 140/4495/BPD";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"SUKAJADI".to_string())
        );
    }

    #[test]
    fn test_parse_kecamatan_digit_name() {
        let text = "\
C.Kabupaten.1) Kabupaten Pasaman Barat Provinsi Sumatera Barat
13.05.04   4 2 x 11 Anam Lingkuang                           3
13.05.04.2001 5 BUKIK KILI";
        let villages = parse_villages(text);
        assert_eq!(villages.len(), 1);
        assert_eq!(villages[0].district, "2 x 11 Anam Lingkuang");
    }

    #[test]
    fn test_parse_kecamatan_name_no_trailing_dash() {
        let text = "\
C.Kabupaten.1) Kabupaten Bandung Provinsi Jawa Barat
31.73.01 60 KECAMATAN BALEENDAH - 7
31.73.01.1001 5 CIPAGARANTU";
        let villages = parse_villages(text);
        assert_eq!(villages.len(), 1);
        assert_eq!(villages[0].district, "KECAMATAN BALEENDAH");
    }

    #[test]
    fn test_extract_district_name_basic() {
        let result = extract_district_name("31.73.01 60 KECAMATAN BALEENDAH - 7");
        assert_eq!(result.name, "KECAMATAN BALEENDAH");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_extract_district_name_trailing_dash_no_space() {
        let result = extract_district_name("31.73.01 60 KECAMATAN BALEENDAH-7");
        assert_eq!(result.name, "KECAMATAN BALEENDAH");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_extract_district_name_no_trailing_count() {
        let result = extract_district_name("31.73.01 60 KECAMATAN BALEENDAH   7");
        assert_eq!(result.name, "KECAMATAN BALEENDAH");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_strip_trailing_count_dash_space_number() {
        assert_eq!(
            strip_trailing_count("KECAMATAN BALEENDAH - 7"),
            "KECAMATAN BALEENDAH"
        );
    }

    #[test]
    fn test_strip_trailing_count_dash_number() {
        assert_eq!(
            strip_trailing_count("KECAMATAN BALEENDAH-7"),
            "KECAMATAN BALEENDAH"
        );
    }

    #[test]
    fn test_strip_trailing_count_space_number() {
        assert_eq!(
            strip_trailing_count("KECAMATAN BALEENDAH 7"),
            "KECAMATAN BALEENDAH"
        );
    }

    #[test]
    fn test_strip_trailing_count_preserves_name_with_digit() {
        assert_eq!(
            strip_trailing_count("2 x 11 Anam Lingkuang 3"),
            "2 x 11 Anam Lingkuang 3"
        );
    }

    #[test]
    fn test_strip_trailing_count_no_trailing_number() {
        assert_eq!(strip_trailing_count("Bakongan"), "Bakongan");
    }

    #[test]
    fn test_has_reference_indicator_digit() {
        assert!(has_reference_indicator("qanun no. 32/2005", 6, 30));
    }

    #[test]
    fn test_has_reference_indicator_nomor() {
        assert!(has_reference_indicator("perbaikan nama desa", 10, 30));
    }

    #[test]
    fn test_has_reference_indicator_no_match() {
        assert!(!has_reference_indicator("uu jaya sejahtera", 3, 30));
    }

    #[test]
    fn test_find_note_boundary_self_validating() {
        let raw = "RAMBONG Semula wil Kec. Bakongan";
        let raw_lower = raw.to_lowercase();
        let m = find_note_boundary(&raw_lower).unwrap();
        assert_eq!(m.pos, 8);
        assert_eq!(m.keyword, "Semula");
    }

    #[test]
    fn test_find_note_boundary_with_reference() {
        let raw = "SUKAMAKMUR UU No. 4/2002";
        let raw_lower = raw.to_lowercase();
        let m = find_note_boundary(&raw_lower).unwrap();
        assert_eq!(m.pos, 11);
        assert_eq!(m.keyword, "UU");
    }

    #[test]
    fn test_find_note_boundary_without_reference() {
        let raw = "UU JAYA";
        let raw_lower = raw.to_lowercase();
        assert!(find_note_boundary(&raw_lower).is_none());
    }

    #[test]
    fn test_pmd_self_validating_without_reference() {
        let raw = "desa jaya pmd";
        let raw_lower = raw.to_lowercase();
        let m = find_note_boundary(&raw_lower).unwrap();
        assert_eq!(m.keyword, "PMD");
    }

    #[test]
    fn test_semula_ocr_typo_semual() {
        let name_re = name_re();
        let after_code = " 2 RAMBONG Semual wil Kec. Bakongan Perda No. 3/2010";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(name.as_ref().map(|e| &e.name), Some(&"RAMBONG".to_string()));
    }

    #[test]
    fn test_pp_with_reference() {
        let name_re = name_re();
        let after_code = " 2 MEKAR JAYA PP No. 6/2010";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"MEKAR JAYA".to_string())
        );
    }

    #[test]
    fn test_berdasarkan_with_reference() {
        let name_re = name_re();
        let after_code = " 3 SUKAMAJU Berdasarkan Perda No. 3/2010";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"SUKAMAJU".to_string())
        );
    }

    #[test]
    fn test_is_word_boundary_at_start() {
        assert!(is_word_boundary("ND Rekom", 0, 2));
    }

    #[test]
    fn test_is_word_boundary_at_end() {
        assert!(is_word_boundary("foo UU", 4, 2));
    }

    #[test]
    fn test_is_word_boundary_inside_word() {
        assert!(!is_word_boundary("poNDok", 2, 2));
        assert!(!is_word_boundary("bandar", 2, 2));
    }

    #[test]
    fn test_find_note_boundary_nd_inside_word_not_matched() {
        let raw = "pondok kahuru";
        let raw_lower = raw.to_lowercase();
        assert!(find_note_boundary(&raw_lower).is_none());
    }

    #[test]
    fn test_find_note_boundary_uu_inside_word_not_matched() {
        let raw = "puuk indah";
        let raw_lower = raw.to_lowercase();
        assert!(find_note_boundary(&raw_lower).is_none());
    }

    #[test]
    fn test_find_note_boundary_nd_at_word_boundary_still_matched() {
        let raw = "sukajadi nd rekom no 140/4495/bpd";
        let raw_lower = raw.to_lowercase();
        let m = find_note_boundary(&raw_lower);
        assert!(m.is_some());
        assert_eq!(m.unwrap().keyword, "ND");
    }

    #[test]
    fn test_parse_villages_basic() {
        let text = "\
C.K.1) Kabupaten Bandung Provinsi Jawa Barat
31.73.01 60 KECAMATAN BALEENDAH - 7
31.73.01.1001 5 CIPAGARANTU
31.73.01.1002 12 MARGASARI";
        let villages = parse_villages(text);
        assert_eq!(villages.len(), 2);
        assert_eq!(villages[0].code, "31.73.01.1001");
        assert_eq!(villages[0].name, "CIPAGARANTU");
        assert_eq!(villages[0].province, "Provinsi Jawa Barat");
        assert_eq!(villages[0].city, "Kabupaten Bandung");
        assert_eq!(villages[1].code, "31.73.01.1002");
        assert_eq!(villages[1].name, "MARGASARI");
    }

    #[test]
    fn test_village_parser_struct() {
        let parser = VillageParser::new();
        let text = "\
C.K.1) Kabupaten Bandung Provinsi Jawa Barat
31.73.01 60 KECAMATAN BALEENDAH - 7
31.73.01.1001 5 CIPAGARANTU
31.73.01.1002 12 MARGASARI";
        let villages = parser.parse(text);
        assert_eq!(villages.len(), 2);
        assert_eq!(villages[0].code, "31.73.01.1001");
        assert_eq!(villages[1].code, "31.73.01.1002");
    }

    #[test]
    fn test_village_code_at_end_of_line() {
        let name_re = name_re();
        let after_code = " 5 CIPAGARANTU";
        let name = extract_village_name(after_code, name_re);
        assert_eq!(
            name.as_ref().map(|e| &e.name),
            Some(&"CIPAGARANTU".to_string())
        );
    }

    #[test]
    fn test_extracted_name_metadata_with_note() {
        let name_re = name_re();
        let after_code = " 2 RAMBONG Semula wil Kec. Bakongan";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "RAMBONG");
        assert_eq!(
            extracted.raw_name.as_deref(),
            Some("RAMBONG Semula wil Kec. Bakongan")
        );
        assert_eq!(extracted.note_keyword.as_deref(), Some("Semula"));
        assert_eq!(extracted.note_boundary, Some(8));
    }

    #[test]
    fn test_extracted_name_metadata_no_note() {
        let name_re = name_re();
        let after_code = " 12 ABADIJAYA";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "ABADIJAYA");
        assert!(extracted.raw_name.is_none());
        assert!(extracted.note_keyword.is_none());
        assert!(extracted.note_boundary.is_none());
    }

    #[test]
    fn test_extracted_name_raw_name_from_truncation() {
        let name_re = name_re();
        let after_code = " 10 A B C D E F";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "A B C D E");
        assert_eq!(extracted.raw_name.as_deref(), Some("A B C D E F"));
        assert!(extracted.note_keyword.is_none());
    }

    #[test]
    fn test_save_parsed_villages_minimal() {
        let villages = vec![VillageRecord {
            code: "31.71.03.1001".to_string(),
            name: "Kemayoran".to_string(),
            district: "Kemayoran".to_string(),
            city: "Jakarta Pusat".to_string(),
            province: "Jakarta".to_string(),
            raw_name: None,
            note_keyword: None,
            note_boundary: None,
            district_note: None,
        }];
        let dir = std::env::temp_dir().join("wilayah_test_parse_minimal");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("parsed.json");
        save_parsed_villages(&villages, ParseOutputDetail::Minimal, &path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["code"], "31.71.03.1001");
        assert_eq!(parsed[0]["name"], "Kemayoran");
        assert!(parsed[0].get("raw_name").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_parsed_villages_with_raw_name() {
        let villages = vec![VillageRecord {
            code: "31.71.03.1001".to_string(),
            name: "RAMBONG".to_string(),
            district: "Bakongan".to_string(),
            city: "Aceh Selatan".to_string(),
            province: "Aceh".to_string(),
            raw_name: Some("RAMBONG Semula wil Kec. Bakongan".to_string()),
            note_keyword: Some("Semula".to_string()),
            note_boundary: Some(8),
            district_note: None,
        }];
        let dir = std::env::temp_dir().join("wilayah_test_parse_raw");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("parsed.json");
        save_parsed_villages(&villages, ParseOutputDetail::WithRawName, &path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed[0]["raw_name"], "RAMBONG Semula wil Kec. Bakongan");
        assert!(parsed[0].get("note_keyword").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_parsed_villages_full() {
        let villages = vec![VillageRecord {
            code: "31.71.03.1001".to_string(),
            name: "RAMBONG".to_string(),
            district: "Bakongan".to_string(),
            city: "Aceh Selatan".to_string(),
            province: "Aceh".to_string(),
            raw_name: Some("RAMBONG Semula wil Kec. Bakongan".to_string()),
            note_keyword: Some("Semula".to_string()),
            note_boundary: Some(8),
            district_note: None,
        }];
        let dir = std::env::temp_dir().join("wilayah_test_parse_full");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("parsed.json");
        save_parsed_villages(&villages, ParseOutputDetail::Full, &path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed[0]["raw_name"], "RAMBONG Semula wil Kec. Bakongan");
        assert_eq!(parsed[0]["note_keyword"], "Semula");
        assert_eq!(parsed[0]["note_boundary"], 8);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_split_on_column_gap_basic() {
        let parts = split_on_column_gap("Bakongan                               7");
        assert_eq!(parts, vec!["Bakongan", "7"]);
    }

    #[test]
    fn test_split_on_column_gap_three_parts() {
        let parts = split_on_column_gap("Makbon    1    14    Semula wil. Provinsi Papua");
        assert_eq!(
            parts,
            vec!["Makbon", "1", "14", "Semula wil. Provinsi Papua"]
        );
    }

    #[test]
    fn test_split_on_column_gap_no_gap() {
        let parts = split_on_column_gap("SinglePart");
        assert_eq!(parts, vec!["SinglePart"]);
    }

    #[test]
    fn test_split_on_column_gap_two_spaces_not_split() {
        let parts = split_on_column_gap("Hello  World");
        assert_eq!(parts, vec!["Hello  World"]);
    }

    #[test]
    fn test_is_kecamatan_code_valid() {
        assert!(is_kecamatan_code("11.01.01"));
        assert!(is_kecamatan_code("96.01.01"));
    }

    #[test]
    fn test_is_kecamatan_code_invalid() {
        assert!(!is_kecamatan_code("11.01"));
        assert!(!is_kecamatan_code("KECAMATAN"));
        assert!(!is_kecamatan_code("11.01.01.2001"));
        assert!(!is_kecamatan_code(""));
    }

    #[test]
    fn test_extract_district_name_columnar_format() {
        let result = extract_district_name("11.01.01 1 Bakongan                               7");
        assert_eq!(result.name, "Bakongan");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_extract_district_name_code_as_name() {
        let result = extract_district_name("96.01.01                    1 Makbon");
        assert_eq!(result.name, "Makbon");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_extract_district_name_with_suffix_note() {
        let result = extract_district_name(
            "96.01.01 1 Makbon    1  14    Semula wil. Provinsi Papua Barat; UU No. 16 Tahun 2025",
        );
        assert_eq!(result.name, "Makbon");
        assert!(result.note.is_some());
        let note = result.note.unwrap();
        assert!(
            note.starts_with("Semula"),
            "note should start with 'Semula': {note}"
        );
    }

    #[test]
    fn test_extract_district_name_note_in_name_column() {
        let result = extract_district_name("96.01.01 1 Abenaho Semula wil Prov.");
        assert_eq!(result.name, "Abenaho");
        assert!(result.note.is_some());
    }

    #[test]
    fn test_strip_district_note_no_keyword() {
        let (name, note) = strip_district_note("Bakongan");
        assert_eq!(name, "Bakongan");
        assert!(note.is_none());
    }

    #[test]
    fn test_strip_district_note_with_keyword() {
        let (name, note) = strip_district_note("Abenaho Semula wil Prov.");
        assert_eq!(name, "Abenaho");
        assert_eq!(note.as_deref(), Some("Semula wil Prov."));
    }

    #[test]
    fn test_extract_suffix_note_no_keyword() {
        assert!(extract_suffix_note("7".to_string()).is_none());
        assert!(extract_suffix_note("1  14".to_string()).is_none());
    }

    #[test]
    fn test_extract_suffix_note_with_keyword() {
        let note = extract_suffix_note("1  14  Semula wil. Provinsi Papua Barat".to_string());
        assert!(note.is_some());
        assert!(note.unwrap().starts_with("Semula"));
    }

    #[test]
    fn test_first_gap_position_with_gap() {
        assert_eq!(
            first_gap_position("Ujong Mangki                             Perbaikan nama"),
            Some(12)
        );
    }

    #[test]
    fn test_first_gap_position_no_gap() {
        assert_eq!(first_gap_position("Keude Bakongan"), None);
    }

    #[test]
    fn test_first_gap_position_two_spaces_not_gap() {
        assert_eq!(first_gap_position("Hello  World"), None);
    }

    #[test]
    fn test_extract_village_name_gap_with_known_keyword() {
        let name_re = name_re();
        let after_code =
            " 1 Ujong Mangki                             Perbaikan nama sesuai Surat Pemkab";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "Ujong Mangki");
        assert_eq!(extracted.note_keyword.as_deref(), Some("Perbaikan"));
    }

    #[test]
    fn test_extract_village_name_gap_with_nagari_suffix() {
        let name_re = name_re();
        let after_code =
            " 1 Pakan Sinayan                                       Nagari Koto Nan IV";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "Pakan Sinayan");
        assert!(extracted.note_keyword.is_none());
    }

    #[test]
    fn test_extract_village_name_gap_with_ordinal() {
        let name_re = name_re();
        let after_code =
            " 1 Pasirdoton                         1. Surat Bupati Sukabumi No. 100/609";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "Pasirdoton");
        assert_eq!(extracted.note_keyword.as_deref(), Some("Surat"));
    }

    #[test]
    fn test_extract_village_name_gap_with_surat_prefix() {
        let name_re = name_re();
        let after_code =
            " 1 Rancaran                      Surat Bup. Padang Lawas Utara No. 141/1154";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "Rancaran");
        assert_eq!(extracted.note_keyword.as_deref(), Some("Surat"));
    }

    #[test]
    fn test_extract_village_name_gap_keyword_in_name_column() {
        let name_re = name_re();
        let after_code =
            " 2 Matang Rayeuk PP                   Semula wil. Kec Idi Rayeuk, Perbaikan nama";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "Matang Rayeuk");
    }

    #[test]
    fn test_extract_village_name_gap_no_keyword_clean_name() {
        let name_re = name_re();
        let after_code = " 1 Gelombang                    Semua wil. Kec. Takeran";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "Gelombang");
    }

    #[test]
    fn test_extract_village_name_gap_with_penggabungan() {
        let name_re = name_re();
        let after_code =
            " 1 Tanjuanggodang                                Penggabungan Kel. Tanjuang Gadang dan Kel. Sungai Pinago";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "Tanjuanggodang");
    }

    #[test]
    fn test_extract_village_name_no_gap_keyword_only() {
        let name_re = name_re();
        let after_code = " 2 RAMBONG Semula wil Kec. Bakongan";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "RAMBONG");
        assert_eq!(extracted.note_keyword.as_deref(), Some("Semula"));
    }

    #[test]
    fn test_extract_village_name_no_gap_no_keyword() {
        let name_re = name_re();
        let after_code = " 12 ABADIJAYA";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "ABADIJAYA");
        assert!(extracted.note_keyword.is_none());
    }

    #[test]
    fn test_extract_village_name_gap_preserves_existing_keyword_detection() {
        let name_re = name_re();
        let after_code =
            " 1 Seuneubok Keuranji                  Semula wil Kec.Bakongan Perda No.3/2010";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "Seuneubok Keuranji");
        assert_eq!(extracted.note_keyword.as_deref(), Some("Semula"));
    }

    #[test]
    fn test_extract_village_name_gap_lowercase_perbaikan() {
        let name_re = name_re();
        let after_code =
            " 1 Teluk Latak                            Perbaikan Spasi, Berdasarka Ketua KPU Bengkalis No Surat";
        let extracted = extract_village_name(after_code, name_re).unwrap();
        assert_eq!(extracted.name, "Teluk Latak");
        assert!(extracted.note_keyword.is_none());
    }
}
