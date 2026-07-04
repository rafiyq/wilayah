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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) kel_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) desa_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) keterangan: Option<String>,
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
    /// The full annotation text from the village line (text after column gap following the name).
    pub(crate) keterangan: Option<String>,
}

/// Result of extracting a district name from a kecamatan line in PDF text.
pub(crate) struct ExtractedDistrict {
    /// The cleaned district name (after column splitting + note stripping).
    pub(crate) name: String,
    /// Annotation text from the kecamatan line (e.g., "Semula wil. Provinsi Papua Barat; UU No. 16 Tahun 2025").
    pub(crate) note: Option<String>,
    /// Number of kelurahan in this kecamatan (0 if explicitly "-").
    pub(crate) kel_count: Option<u32>,
    /// Number of desa in this kecamatan (0 if explicitly "-").
    pub(crate) desa_count: Option<u32>,
}

/// A parsed district (kecamatan) record for JSON output.
#[derive(serde::Serialize, Clone)]
pub(crate) struct DistrictRecord {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) city: String,
    pub(crate) province: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) district_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) kel_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) desa_count: Option<u32>,
}

/// A parsed province record for JSON output.
#[derive(serde::Serialize, Clone)]
pub(crate) struct ProvinceRecord {
    pub(crate) code: String,
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ibukota: Option<String>,
    pub(crate) kab_count: u32,
    pub(crate) kota_count: u32,
    pub(crate) kec_count: u32,
    pub(crate) kel_count: u32,
    pub(crate) desa_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) luas_km2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) penduduk: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) island_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) keterangan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) population_male: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) population_female: Option<u64>,
}

/// A parsed city (kabupaten/kota) record for JSON output.
#[derive(serde::Serialize, Clone)]
pub(crate) struct CityRecord {
    pub(crate) code: String,
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ibukota: Option<String>,
    pub(crate) kec_count: u32,
    pub(crate) kel_count: u32,
    pub(crate) desa_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) luas_km2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) penduduk: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) keterangan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) population_male: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) population_female: Option<u64>,
}

/// A parsed city-level island count from Section D.b of the PDF.
#[derive(serde::Serialize, Clone)]
pub(crate) struct IslandSummary {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) province: String,
    pub(crate) island_count: u32,
}

/// A parsed individual island record from Section D.c of the PDF.
#[derive(serde::Serialize, Clone)]
pub(crate) struct IslandRecord {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) kabupaten_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) latitude: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) longitude: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) area_km2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) keterangan: Option<String>,
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

/// Maximum bytes after a keyword to scan for a reference indicator.
const REFERENCE_WINDOW: usize = 30;

/// Minimum leading spaces for a continuation line to be keterangan text.
const KETERANGAN_CONTINUATION_INDENT: usize = 20;

/// Maximum trailing digits that look like a count (not part of name).
const MAX_TRAILING_COUNT_DIGITS: usize = 3;

/// Minimum word length (chars) for trailing-period stripping.
const MIN_WORD_LEN_FOR_PERIOD_STRIP: usize = 4;

/// Minimum tokens expected in Section A (province table) rows.
const MIN_TOKENS_SECTION_A: usize = 8;

/// Minimum tokens expected in header lines (province/city rest).
const MIN_TOKENS_HEADER: usize = 3;

/// Minimum numeric fields expected in province rest.
const MIN_NUM_FIELDS_PROVINCE: usize = 5;

/// Minimum numeric fields expected in city rest.
const MIN_NUM_FIELDS_CITY: usize = 2;

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

/// Find the earliest occurrence of any keyword in `text`, optionally requiring a validator.
fn find_earliest_keyword(
    text: &str,
    keywords: &[&'static str],
    validator: impl Fn(&str, usize, &str) -> bool,
) -> Option<(usize, &'static str)> {
    let text_lower = &text.to_lowercase();
    let mut matches: Vec<(usize, &'static str)> = Vec::new();
    for kw in keywords {
        let kw_lower = kw.to_lowercase();
        let kw_len = kw_lower.len();
        if let Some(pos) = text_lower.find(&kw_lower) {
            if is_word_boundary(text_lower, pos, kw_len) && validator(text_lower, pos, &kw_lower) {
                matches.push((pos, *kw));
            }
        }
    }
    matches.into_iter().min_by_key(|(pos, _)| *pos)
}

/// Find the earliest note boundary in `raw` by checking all note keywords.
///
/// Self-validating keywords always mark a note boundary.
/// Reference-validated keywords only mark a boundary if followed by a
/// reference-like pattern within 30 characters.
///
/// Returns the earliest match if found, or `None` if no note keyword was detected.
fn find_note_boundary(raw_lower: &str) -> Option<NoteMatch> {
    let self_match = find_earliest_keyword(raw_lower, SELF_VALIDATING_KEYWORDS, |_, _, _| true);
    let ref_match =
        find_earliest_keyword(raw_lower, REFERENCE_VALIDATED_KEYWORDS, |text, pos, kw| {
            has_reference_indicator(text, pos + kw.len(), REFERENCE_WINDOW)
        });
    let best = match (self_match, ref_match) {
        (Some((sp, _)), Some((rp, rk))) if rp < sp => Some((rp, rk)),
        (Some((sp, sk)), _) => Some((sp, sk)),
        (_, Some((rp, rk))) => Some((rp, rk)),
        _ => None,
    };
    best.map(|(pos, keyword)| NoteMatch { pos, keyword })
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
        let mut current_kel_count: Option<u32> = None;
        let mut current_desa_count: Option<u32> = None;

        for line in text.lines() {
            if let Some(header) = parse_section_header(line, &self.section_header_re) {
                current_province = header.province;
                current_city = header.city;
                current_district_code.clear();
                current_district_name.clear();
                current_district_note = None;
                current_kel_count = None;
                current_desa_count = None;
                continue;
            }

            if let Some(cap) = self.kecamatan_code_re.captures(line) {
                current_district_code = cap.get(1).unwrap().as_str().to_string();
                let after_prefix = &line[cap.get(0).unwrap().start()..];
                let extracted = extract_district_name(after_prefix);
                current_district_name = extracted.name;
                current_district_note = extracted.note;
                current_kel_count = extracted.kel_count;
                current_desa_count = extracted.desa_count;
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
                        kel_count: current_kel_count,
                        desa_count: current_desa_count,
                        keterangan: extracted.keterangan,
                    });
                } else {
                    eprintln!(
                        "  [warn] Skipping village line, could not parse name: {}",
                        code_str
                    );
                }
                continue;
            }

            if let Some(cont) = is_keterangan_continuation(line) {
                append_keterangan(&mut villages, &cont);
            }
        }

        eprintln!("Parsed {} villages", villages.len());

        fixup_district_notes(&mut villages);

        villages
    }
}

/// If a district_note looks like a suffix appended to the district name rather than
/// a real annotation (doesn't match known note keywords), merge it back into the name.
fn fixup_district_notes(villages: &mut [VillageRecord]) {
    let lowercase_note_prefixes: Vec<String> = DISTRICT_NOTE_KEYWORDS
        .iter()
        .map(|kw| kw.to_lowercase())
        .collect();

    for v in villages {
        if let Some(ref note) = v.district_note {
            let note_bytes = note.as_bytes();
            if note_bytes.first().is_none_or(|c| c.is_ascii_lowercase()) {
                let note_lower = note.to_lowercase();
                let is_real_note = lowercase_note_prefixes
                    .iter()
                    .any(|kw| note_lower.starts_with(kw.as_str()));
                if !is_real_note {
                    v.district = format!("{}{}", v.district, note);
                    v.district_note = None;
                }
            }
        }
    }
}

/// Parse village records from extracted PDF text.
///
/// Convenience wrapper around [`VillageParser::parse`].
pub(crate) fn parse_villages(text: &str) -> Vec<VillageRecord> {
    VillageParser::new().parse(text)
}

/// Extract unique district (kecamatan) records from parsed village records.
///
/// Each district appears once, with its kel/desa counts and optional district_note.
pub(crate) fn extract_districts(villages: &[VillageRecord]) -> Vec<DistrictRecord> {
    let mut seen = std::collections::HashSet::new();
    let mut districts = Vec::new();
    for v in villages {
        if seen.insert(v.code[..8].to_string()) {
            districts.push(DistrictRecord {
                code: v.code[..8].to_string(),
                name: v.district.clone(),
                city: v.city.clone(),
                province: v.province.clone(),
                district_note: v.district_note.clone(),
                kel_count: v.kel_count,
                desa_count: v.desa_count,
            });
        }
    }
    districts
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

trait HasKeterangan {
    fn keterangan_mut(&mut self) -> &mut Option<String>;
}

fn append_keterangan<T: HasKeterangan>(items: &mut [T], text: &str) {
    if let Some(last) = items.last_mut() {
        let ket = last.keterangan_mut();
        if let Some(ref mut kt) = *ket {
            kt.push(' ');
            kt.push_str(text);
        } else {
            *ket = Some(text.to_string());
        }
    }
}

impl HasKeterangan for VillageRecord {
    fn keterangan_mut(&mut self) -> &mut Option<String> {
        &mut self.keterangan
    }
}

impl HasKeterangan for CCityHeader {
    fn keterangan_mut(&mut self) -> &mut Option<String> {
        &mut self.keterangan
    }
}

/// Find the byte position where the first column gap (3+ consecutive spaces) starts.
///
/// Returns `Some(pos)` where `pos` is the byte offset of the first space in the
/// first 3+ consecutive space run. Returns `None` if the text has no column gap.
///
/// This is used in `extract_village_name` to detect Format 2 PDF lines where the
/// village name and annotation text are separated by a wide space gap.
/// Detect a keterangan continuation line in the PDF text.
///
/// Keterangan continuation lines are deeply indented (20+ leading spaces) and
/// contain annotation text that continues from the previous village line.
/// They do not start with a village code or a number in the first 10 characters.
/// Form-feed characters (page breaks) indicate non-continuation lines.
fn is_keterangan_continuation(line: &str) -> Option<String> {
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('\x0c') {
        return None;
    }
    let leading = line.len() - line.trim_start().len();
    if leading < KETERANGAN_CONTINUATION_INDENT {
        return None;
    }
    let content = trimmed.trim_start();
    if content.is_empty() {
        return None;
    }
    if content.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    Some(content.to_string())
}

fn find_column_gap(s: &str) -> Option<usize> {
    let mut consecutive = 0usize;
    let mut gap_start: Option<usize> = None;
    for (i, c) in s.char_indices() {
        if c == ' ' {
            if consecutive == 0 {
                gap_start = Some(i);
            }
            consecutive += 1;
        } else {
            if consecutive >= COLUMN_GAP_SPACES {
                return gap_start;
            }
            consecutive = 0;
            gap_start = None;
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

    let (kel_count, desa_count, note_suffix_start) = parse_suffix_counts(&suffix_parts);

    let remaining_suffix: Vec<&str> = suffix_parts[note_suffix_start..].to_vec();
    let suffix_note = if !remaining_suffix.is_empty() {
        extract_suffix_note(remaining_suffix.join(" "))
    } else {
        None
    };

    let note = name_note.or(suffix_note);

    ExtractedDistrict {
        name: cleaned_name.to_string(),
        note,
        kel_count,
        desa_count,
    }
}

/// Apply note keyword stripping to a district name (for cases where the note keyword
/// is embedded in the name column before a column gap, e.g., "Abenaho Semula wil Prov.").
fn strip_district_note(name: &str) -> (&str, Option<String>) {
    let name_lower = name.to_lowercase();
    for keyword in DISTRICT_NOTE_KEYWORDS {
        let kw_lower = keyword.to_lowercase();
        if let Some(pos) = name_lower.find(&kw_lower) {
            if !is_word_boundary(&name_lower, pos, kw_lower.len()) {
                continue;
            }
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
    find_earliest_keyword(&suffix, DISTRICT_NOTE_KEYWORDS, |_, _, _| true)
        .map(|(pos, _)| suffix[pos..].trim().to_string())
}

/// Parse kel/desa counts from the suffix parts after the district name.
///
/// Suffix parts come from `split_on_column_gap` after the name column. The format is:
/// - Listing (format 2): `["-", "7"]` or `["6", "-"]` or `["2", "14"]`
///   First is KEL count (or "-" for 0), second is DESA count (or "-" for 0)
/// - Header (format 1): `["7"]` — single count (desa for kabupaten, kel for kota)
/// - With note: `["-", "14", "Semula", "wil.", "Provinsi..."]` — counts first, then note text
///
/// Returns `(kel_count, desa_count, note_start_index)` where counts are None if not found,
/// and `note_start_index` is the index into suffix_parts where non-count text begins.
fn parse_suffix_counts(suffix_parts: &[&str]) -> (Option<u32>, Option<u32>, usize) {
    if suffix_parts.is_empty() {
        return (None, None, 0);
    }

    let parse_count = |s: &str| -> Option<u32> {
        if s == "-" {
            Some(0)
        } else {
            s.parse::<u32>().ok()
        }
    };

    let first = parse_count(suffix_parts[0]);

    if let Some(kel) = first {
        if suffix_parts.len() > 1 {
            if let Some(desa) = parse_count(suffix_parts[1]) {
                return (Some(kel), Some(desa), 2);
            }
        }
        return (Some(kel), None, 1);
    }

    (None, None, 0)
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
    if digit_count == 0 || digit_count > MAX_TRAILING_COUNT_DIGITS {
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

fn strip_trailing_period(name: &mut String) {
    if !name.ends_with('.') {
        return;
    }
    let before_period = &name[..name.len() - 1];
    let last_word = before_period.split_whitespace().last().unwrap_or("");
    if last_word.len() >= MIN_WORD_LEN_FOR_PERIOD_STRIP {
        name.truncate(name.len() - 1);
    }
}

fn capitalize_all_lowercase(name: &mut String) {
    if name.is_empty() || !name.chars().all(|c| !c.is_uppercase()) {
        return;
    }
    let mut result = String::with_capacity(name.len());
    let mut capitalize_next = true;
    for c in name.chars() {
        if c == ' ' {
            capitalize_next = true;
            result.push(c);
        } else if capitalize_next {
            result.extend(c.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    *name = result;
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

/// Resolve the cleaned name and optional keterangan by choosing the cutoff between
/// a column-gap boundary and a note-keyword boundary (whichever comes first).
fn resolve_name_and_keterangan<'a>(
    raw: &'a str,
    note_match: &Option<NoteMatch>,
    gap_pos: Option<usize>,
) -> Option<(&'a str, Option<String>)> {
    if let Some(gp) = gap_pos {
        let keyword_pos = note_match.as_ref().map_or(usize::MAX, |n| n.pos);
        if keyword_pos < gp {
            let c = raw[..keyword_pos].trim();
            if c.is_empty() {
                return None;
            }
            Some((c, Some(raw[keyword_pos..].trim().to_string())))
        } else {
            let c = raw[..gp].trim();
            if c.is_empty() {
                return None;
            }
            let tail = raw[gp..].trim();
            Some((
                c,
                if tail.is_empty() {
                    None
                } else {
                    Some(tail.to_string())
                },
            ))
        }
    } else {
        match note_match {
            Some(note) => {
                let c = raw[..note.pos].trim();
                if c.is_empty() {
                    return None;
                }
                Some((c, Some(raw[note.pos..].trim().to_string())))
            }
            None => Some((raw, None)),
        }
    }
}

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
    let gap_pos = find_column_gap(raw);

    let (cleaned, keterangan_text) = resolve_name_and_keterangan(raw, &note_match, gap_pos)?;

    let mut truncated: String = cleaned
        .split_whitespace()
        .take(MAX_NAME_WORDS)
        .collect::<Vec<_>>()
        .join(" ");

    if truncated.is_empty() {
        return None;
    }

    strip_trailing_period(&mut truncated);
    capitalize_all_lowercase(&mut truncated);

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
        keterangan: keterangan_text,
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

fn parse_indonesian_int(s: &str) -> Option<u64> {
    let cleaned: String = s.chars().filter(|c| *c != '.').collect();
    cleaned.parse().ok()
}

fn parse_indonesian_float(s: &str) -> Option<f64> {
    let s = s.trim();
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c == '.' {
                '\x00'
            } else if c == ',' {
                '.'
            } else {
                c
            }
        })
        .filter(|c| *c != '\x00')
        .collect();
    cleaned.parse().ok()
}

struct SectionAProvince {
    code: String,
    name: String,
    kab_count: u32,
    kota_count: u32,
    kec_count: u32,
    kel_count: u32,
    desa_count: u32,
    luas_km2: Option<f64>,
    penduduk: Option<u64>,
    island_count: Option<u32>,
}

#[allow(dead_code)]
struct CProvinceHeader {
    code: String,
    name: String,
    ibukota: Option<String>,
    keterangan: Option<String>,
}

struct CCityHeader {
    code: String,
    name: String,
    ibukota: Option<String>,
    kec_count: u32,
    kel_count: u32,
    desa_count: u32,
    luas_km2: Option<f64>,
    penduduk: Option<u64>,
    keterangan: Option<String>,
}

struct SectionEEntry {
    code: String,
    male: Option<u64>,
    female: Option<u64>,
    #[allow(dead_code)]
    total: Option<u64>,
}

fn parse_section_a(text: &str) -> Vec<SectionAProvince> {
    let mut provinces = Vec::new();
    let re = regex::Regex::new(r"^\s*\d+\s+(\d{2})\s+(\S+(?:\s+\S+)*?)\s{2,}(\S.*)$").unwrap();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("NO")
            || line.starts_with("A.")
            || line.starts_with("JUMLAH")
            || line.starts_with("KETERANGAN")
            || line.starts_with("*")
            || line.starts_with("**")
            || line.starts_with("***")
            || line.starts_with("****")
            || line.chars().next().is_none_or(|c| !c.is_ascii_digit())
        {
            continue;
        }
        if let Some(cap) = re.captures(line) {
            let code = cap[1].to_string();
            let name = cap[2].trim().to_string();
            let tail = cap[3].trim();

            let tokens: Vec<&str> = tail.split_whitespace().collect();
            if tokens.len() < MIN_TOKENS_SECTION_A {
                eprintln!(
                    "  [warn] Section A: row {} has {} tokens, expected at least {}",
                    name,
                    tokens.len(),
                    MIN_TOKENS_SECTION_A
                );
                continue;
            }

            let kab_count = tokens[0].parse().unwrap_or(0);
            let kota_count = tokens[1].parse().unwrap_or(0);
            let kec_count = tokens[2].parse().unwrap_or(0);
            let kel_count = parse_indonesian_int(tokens[3]).unwrap_or(0) as u32;
            let desa_count = parse_indonesian_int(tokens[4]).unwrap_or(0) as u32;
            let luas_km2 = parse_indonesian_float(tokens[5]);
            let penduduk = parse_indonesian_int(tokens[6]);
            let island_count = parse_indonesian_int(&tokens[7].replace(',', "")).map(|v| v as u32);

            provinces.push(SectionAProvince {
                code,
                name,
                kab_count,
                kota_count,
                kec_count,
                kel_count,
                desa_count,
                luas_km2,
                penduduk,
                island_count,
            });
        } else {
            eprintln!(
                "  [warn] Section A: could not parse line: {:?}",
                &line[..line.len().min(80)]
            );
        }
    }
    provinces
}

fn parse_c_province_headers(text: &str) -> Vec<CProvinceHeader> {
    let mut headers = Vec::new();

    let roman_re = regex::Regex::new(
        r"^\s*(?:I{1,3}|IV|V?I{1,3}|IX|X{1,3}V?I{0,3}|XXXI{0,2})\s+(\d{2})\s+([A-Za-z].+)$",
    )
    .unwrap();

    let arabic_re = regex::Regex::new(r"^\s+(9[3-6])\s+([A-Za-z].+)$").unwrap();

    for line in text.lines() {
        if let Some(cap) = roman_re.captures(line) {
            let code = cap[1].to_string();
            let rest = cap[2].trim();
            if let Some((name, ibukota, keterangan)) = parse_province_rest(rest) {
                headers.push(CProvinceHeader {
                    code,
                    name,
                    ibukota,
                    keterangan,
                });
            }
            continue;
        }
        if let Some(cap) = arabic_re.captures(line) {
            let code = cap[1].to_string();
            let rest = cap[2].trim();
            if let Some((name, ibukota, keterangan)) = parse_province_rest(rest) {
                headers.push(CProvinceHeader {
                    code,
                    name,
                    ibukota,
                    keterangan,
                });
            }
        }
    }
    headers
}

fn find_keterangan_in_num_fields(num_fields: &[&str]) -> Option<String> {
    let ket_pos = num_fields.iter().rposition(|t| !is_likely_number(t));
    if let Some(kp) = ket_pos {
        let ket = num_fields[kp..].join(" ");
        if ket.is_empty() {
            None
        } else {
            Some(ket)
        }
    } else {
        None
    }
}

fn parse_province_rest(rest: &str) -> Option<(String, Option<String>, Option<String>)> {
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    if tokens.len() < MIN_TOKENS_HEADER {
        eprintln!(
            "  [warn] parse_province_rest: too few tokens ({}) in: {:?}",
            tokens.len(),
            &rest[..rest.len().min(80)]
        );
        return None;
    }

    let gap_pos = find_first_large_gap(rest);
    if let Some(gp) = gap_pos {
        let before_gap = rest[..gp].trim_end();
        let after_gap = rest[gp..].trim_start();
        let name = before_gap.to_string();

        let after_tokens: Vec<&str> = after_gap.split_whitespace().collect();
        let ib_end = after_tokens
            .iter()
            .position(|t| is_likely_number(t))
            .unwrap_or(after_tokens.len());
        let ibukota = if ib_end > 0 {
            Some(after_tokens[..ib_end].join(" "))
        } else {
            None
        };

        let num_start_in_after = after_tokens
            .iter()
            .position(|t| is_likely_number(t))
            .unwrap_or(after_tokens.len());
        let num_fields: Vec<&str> = after_tokens[num_start_in_after..].to_vec();
        let keterangan = find_keterangan_in_num_fields(&num_fields);

        return Some((name, ibukota, keterangan));
    }

    let num_start = find_numeric_start(&tokens);
    if num_start < 1 {
        eprintln!(
            "  [warn] parse_province_rest: no numeric fields in: {:?}",
            &rest[..rest.len().min(80)]
        );
        return None;
    }

    let name = tokens[..num_start].join(" ");
    let num_fields = &tokens[num_start..];
    if num_fields.len() < MIN_NUM_FIELDS_PROVINCE {
        eprintln!(
            "  [warn] parse_province_rest: {} num fields, expected at least {} in: {:?}",
            num_fields.len(),
            MIN_NUM_FIELDS_PROVINCE,
            &rest[..rest.len().min(80)]
        );
        return None;
    }

    let keterangan = find_keterangan_in_num_fields(num_fields);
    Some((name, None, keterangan))
}

fn parse_c_city_headers(text: &str) -> Vec<CCityHeader> {
    let mut cities: Vec<CCityHeader> = Vec::new();
    let mut current_keterangan_continuation = false;

    let city_re = regex::Regex::new(r"^\s+(\d{2}\.\d{2})\s+\d+\s+(.+)$").unwrap();

    for line in text.lines() {
        let trimmed = line.trim();

        if current_keterangan_continuation && is_keterangan_continuation(line).is_some() {
            append_keterangan(&mut cities, trimmed);
            continue;
        }
        current_keterangan_continuation = false;

        if let Some(cap) = city_re.captures(line) {
            let code = cap[1].to_string();
            let rest = cap[2].trim();

            let tokens: Vec<&str> = rest.split_whitespace().collect();
            if tokens.len() < MIN_TOKENS_HEADER {
                eprintln!(
                    "  [warn] parse_c_city_headers: too few tokens ({}) in: {:?}",
                    tokens.len(),
                    &rest[..rest.len().min(60)]
                );
                continue;
            }

            let type_idx = tokens
                .iter()
                .position(|t| *t == "Kabupaten" || *t == "Kab" || *t == "Kota");
            let type_idx = match type_idx {
                Some(i) => i,
                None => {
                    eprintln!(
                        "  [warn] parse_c_city_headers: no Kabupaten/Kota label in: {:?}",
                        &rest[..rest.len().min(60)]
                    );
                    continue;
                }
            };

            let num_start = find_numeric_start(&tokens);
            if num_start <= type_idx + 1 {
                eprintln!(
                    "  [warn] parse_c_city_headers: num_start ({}) <= type_idx+1 ({}) for code {}",
                    num_start,
                    type_idx + 1,
                    code
                );
                continue;
            }

            let num_fields = &tokens[num_start..];

            if num_fields.len() < MIN_NUM_FIELDS_CITY {
                eprintln!("  [warn] parse_c_city_headers: {} num fields, expected at least {} for code {}",
                    num_fields.len(), MIN_NUM_FIELDS_CITY, code);
                continue;
            }

            let (name, ibukota) = split_name_ibukota_by_gap(rest, type_idx, num_start);

            let (ibukota_val, kec_count, kel_count, desa_count, luas_km2, penduduk, keterangan) =
                parse_city_fields(num_fields, ibukota);

            current_keterangan_continuation = keterangan.is_some();

            cities.push(CCityHeader {
                code,
                name,
                ibukota: ibukota_val,
                kec_count,
                kel_count,
                desa_count,
                luas_km2,
                penduduk,
                keterangan,
            });
        }
    }
    cities
}

fn find_numeric_start(tokens: &[&str]) -> usize {
    for i in 1..tokens.len() {
        if i + 1 < tokens.len() && is_likely_number(tokens[i]) && is_likely_number(tokens[i + 1]) {
            return i;
        }
    }
    for (i, token) in tokens.iter().enumerate().skip(2) {
        if is_likely_number(token) {
            return i;
        }
    }
    tokens.len()
}

fn is_likely_number(token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let first = token.chars().next().unwrap();
    first.is_ascii_digit()
}

fn split_name_ibukota_by_gap(
    rest: &str,
    type_idx: usize,
    num_start: usize,
) -> (String, Option<String>) {
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    if tokens.is_empty() || type_idx >= tokens.len() {
        return (String::new(), None);
    }

    let first_gap_pos = find_first_large_gap(rest);
    if let Some(gap_pos) = first_gap_pos {
        let before_gap: String = rest[..gap_pos].trim().to_string();
        let after_gap = rest[gap_pos..].trim_start();
        let after_tokens: Vec<&str> = after_gap.split_whitespace().collect();

        let name = before_gap;
        let ibukota = if !after_tokens.is_empty() {
            let ib_end = after_tokens
                .iter()
                .position(|t| is_likely_number(t))
                .unwrap_or(after_tokens.len());
            if ib_end > 0 {
                Some(after_tokens[..ib_end].join(" "))
            } else {
                None
            }
        } else {
            None
        };
        return (name, ibukota);
    }

    let city_type = tokens[type_idx];
    if city_type == "Kota" {
        let name = tokens[type_idx..num_start].join(" ");
        let ibukota = if num_start > type_idx + 1 {
            Some(tokens[type_idx + 1..num_start].join(" "))
        } else {
            None
        };
        return (name, ibukota);
    }

    let name = tokens[type_idx..num_start].join(" ");
    (name, None)
}

fn find_first_large_gap(s: &str) -> Option<usize> {
    find_column_gap(s)
}

type CityFields = (
    Option<String>,
    u32,
    u32,
    u32,
    Option<f64>,
    Option<u64>,
    Option<String>,
);

fn parse_city_fields(tokens: &[&str], pre_ibukota: Option<String>) -> CityFields {
    let mut pos = 0;

    let ibukota = if pre_ibukota.is_some() {
        pre_ibukota
    } else if pos < tokens.len() && !is_likely_number(tokens[pos]) {
        let mut ib_parts = Vec::new();
        while pos < tokens.len() && !is_likely_number(tokens[pos]) {
            ib_parts.push(tokens[pos]);
            pos += 1;
        }
        let ib = ib_parts.join(" ");
        if ib.is_empty() {
            None
        } else {
            Some(ib)
        }
    } else {
        None
    };

    let kec_count = if pos < tokens.len() {
        let v = parse_indonesian_int(tokens[pos]).unwrap_or(0) as u32;
        pos += 1;
        v
    } else {
        0
    };

    let (kel_count, desa_count) = if pos + 1 < tokens.len() {
        let a = parse_indonesian_int(tokens[pos]).unwrap_or(0) as u32;
        let b = parse_indonesian_int(tokens[pos + 1]).unwrap_or(0) as u32;
        pos += 2;
        (a, b)
    } else if pos < tokens.len() {
        let a = parse_indonesian_int(tokens[pos]).unwrap_or(0) as u32;
        pos += 1;
        (a, 0)
    } else {
        (0, 0)
    };

    let luas_km2 = if pos < tokens.len() {
        let v = parse_indonesian_float(tokens[pos]);
        pos += 1;
        v
    } else {
        None
    };

    let penduduk = if pos < tokens.len() {
        let v = parse_indonesian_int(tokens[pos]);
        pos += 1;
        v
    } else {
        None
    };

    let keterangan = if pos < tokens.len() {
        let ket = tokens[pos..].join(" ");
        if ket.is_empty() {
            None
        } else {
            Some(ket)
        }
    } else {
        None
    };

    (
        ibukota, kec_count, kel_count, desa_count, luas_km2, penduduk, keterangan,
    )
}

fn parse_section_e(text: &str) -> Vec<SectionEEntry> {
    let mut entries = Vec::new();
    let re = regex::Regex::new(
        r"^\s*(\d{2}(?:\.\d{2})?)\s+(.+?)\s+([\w'.\d,]+)\s+([\w'.\d,]+)\s+([\w'.\d,]+)\s*$",
    )
    .unwrap();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("Kode")
            || trimmed.starts_with("1")
                && trimmed.contains("2")
                && trimmed.contains("3")
                && trimmed.contains("4")
                && trimmed.contains("5")
                && !trimmed.contains("Kabupaten")
                && !trimmed.contains("Provinsi")
                && !trimmed.contains("Kota")
                && !trimmed.contains("Papua")
                && !trimmed.contains("Aceh")
        {
            continue;
        }
        if trimmed.starts_with("INDONESIA")
            || trimmed.starts_with("Total")
            || trimmed.starts_with("*)")
            || trimmed.starts_with("Salinan")
            || trimmed.starts_with("Kepala")
            || trimmed.starts_with("NIP")
            || trimmed.starts_with("As")
            || trimmed.starts_with("Pe ")
            || trimmed.starts_with("MENTERI")
            || trimmed.starts_with("ttd")
        {
            continue;
        }
        if let Some(cap) = re.captures(trimmed) {
            let code = cap[1].to_string();
            let male_str = &cap[3];
            let female_str = &cap[4];
            let total_str = &cap[5];
            let male = recover_ocr_number(male_str);
            let female = recover_ocr_number(female_str);
            let total = recover_ocr_number(total_str);
            entries.push(SectionEEntry {
                code,
                male,
                female,
                total,
            });
        }
    }
    entries
}

fn recover_ocr_number(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(v) = parse_indonesian_int(s) {
        return Some(v);
    }
    let mut chars: Vec<char> = s.chars().collect();
    if !chars.is_empty() {
        let first = chars[0];
        if first == '\'' || first == 'r' || first == 'l' {
            chars[0] = '1';
        }
        if chars.len() > 1
            && (chars[1] == 't' || chars[1] == 'l')
            && chars[0] == '1'
            && chars[2].is_ascii_digit()
        {
            chars.remove(1);
        }
    }
    let repaired: String = chars
        .into_iter()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect();
    if repaired.is_empty() {
        return None;
    }
    parse_indonesian_int(&repaired)
}

fn build_population_maps(
    section_e: &[SectionEEntry],
) -> (
    std::collections::HashMap<String, u64>,
    std::collections::HashMap<String, u64>,
) {
    let male_map = section_e
        .iter()
        .filter_map(|e| e.male.map(|m| (e.code.clone(), m)))
        .collect();
    let female_map = section_e
        .iter()
        .filter_map(|e| e.female.map(|f| (e.code.clone(), f)))
        .collect();
    (male_map, female_map)
}

pub(crate) fn extract_provinces(text: &str) -> Vec<ProvinceRecord> {
    let section_a = parse_section_a(text);
    let c_headers = parse_c_province_headers(text);
    let section_e = parse_section_e(text);

    let ibukota_map: std::collections::HashMap<String, String> = c_headers
        .iter()
        .filter_map(|h| h.ibukota.as_ref().map(|ib| (h.code.clone(), ib.clone())))
        .collect();

    let ket_map: std::collections::HashMap<String, String> = c_headers
        .iter()
        .filter_map(|h| h.keterangan.as_ref().map(|k| (h.code.clone(), k.clone())))
        .collect();

    let (male_map, female_map) = build_population_maps(&section_e);

    section_a
        .into_iter()
        .map(|prov| {
            let ibukota = ibukota_map.get(&prov.code).cloned();
            let keterangan = ket_map.get(&prov.code).cloned();
            let population_male = male_map.get(&prov.code).copied();
            let population_female = female_map.get(&prov.code).copied();
            ProvinceRecord {
                code: prov.code,
                name: prov.name,
                ibukota,
                keterangan,
                kab_count: prov.kab_count,
                kota_count: prov.kota_count,
                kec_count: prov.kec_count,
                kel_count: prov.kel_count,
                desa_count: prov.desa_count,
                luas_km2: prov.luas_km2,
                penduduk: prov.penduduk,
                island_count: prov.island_count,
                population_male,
                population_female,
            }
        })
        .collect()
}

pub(crate) fn extract_cities(text: &str) -> Vec<CityRecord> {
    let c_cities = parse_c_city_headers(text);
    let section_e = parse_section_e(text);

    let (male_map, female_map) = build_population_maps(&section_e);

    c_cities
        .into_iter()
        .map(|city| {
            let population_male = male_map.get(&city.code).copied();
            let population_female = female_map.get(&city.code).copied();
            CityRecord {
                code: city.code,
                name: city.name,
                ibukota: city.ibukota,
                kec_count: city.kec_count,
                kel_count: city.kel_count,
                desa_count: city.desa_count,
                luas_km2: city.luas_km2,
                penduduk: city.penduduk,
                keterangan: city.keterangan,
                population_male,
                population_female,
            }
        })
        .collect()
}

/// Save parsed province records to a JSON file.
pub(crate) fn save_parsed_provinces(
    provinces: &[ProvinceRecord],
    path: &Path,
) -> Result<(), super::PipelineError> {
    use super::PipelineResultExt;
    let json_str =
        serde_json::to_string_pretty(provinces).ctx("failed to serialize parsed provinces")?;
    std::fs::write(path, json_str).ctx("failed to write parsed provinces JSON")?;
    eprintln!("Saved {} parsed provinces to {:?}", provinces.len(), path);
    Ok(())
}

/// Save parsed city (kabupaten/kota) records to a JSON file.
pub(crate) fn save_parsed_cities(
    cities: &[CityRecord],
    path: &Path,
) -> Result<(), super::PipelineError> {
    use super::PipelineResultExt;
    let json_str = serde_json::to_string_pretty(cities).ctx("failed to serialize parsed cities")?;
    std::fs::write(path, json_str).ctx("failed to write parsed cities JSON")?;
    eprintln!("Saved {} parsed cities to {:?}", cities.len(), path);
    Ok(())
}

/// Save parsed village records to a JSON file.
///
/// The level of detail is controlled by `detail`:
/// - `Minimal`: code + cleaned name + district + city + province
/// - `WithRawName`: adds `raw_name`, `district_note`, `kel_count`, `desa_count`, `keterangan` fields
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
                kel_count: None,
                desa_count: None,
                keterangan: None,
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
                kel_count: v.kel_count,
                desa_count: v.desa_count,
                keterangan: v.keterangan.clone(),
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

/// Save parsed district (kecamatan) records to a JSON file.
pub(crate) fn save_parsed_districts(
    districts: &[DistrictRecord],
    path: &Path,
) -> Result<(), super::PipelineError> {
    use super::PipelineResultExt;
    let json_str =
        serde_json::to_string_pretty(districts).ctx("failed to serialize parsed districts")?;
    std::fs::write(path, json_str).ctx("failed to write parsed districts JSON")?;
    eprintln!("Saved {} parsed districts to {:?}", districts.len(), path);
    Ok(())
}

fn parse_section_db(text: &str) -> Vec<IslandSummary> {
    let mut results = Vec::new();
    let province_re = regex::Regex::new(r"^\s*D\.b\.\d+\)\s+Provinsi\s+(.+)$").unwrap();
    let city_re =
        regex::Regex::new(r"^\s*\d+\s+(\d{2}\.\d{2})\s+(\S+(?:\s+\S+)*?)\s{2,}(\S.*)$").unwrap();

    let mut current_province = String::new();

    for line in text.lines() {
        if let Some(cap) = province_re.captures(line) {
            current_province = cap[1].trim().to_string();
            continue;
        }

        if let Some(cap) = city_re.captures(line) {
            let code = cap[1].to_string();
            let name = cap[2].trim().to_string();
            let tail = cap[3].trim();
            let island_count: u32 = parse_indonesian_int(tail).unwrap_or(0) as u32;
            if island_count == 0 {
                continue;
            }
            results.push(IslandSummary {
                code,
                name,
                province: current_province.clone(),
                island_count,
            });
        }
    }

    results
}

fn coord_regex() -> regex::Regex {
    regex::Regex::new(
        r#"(\d{1,2})°(\d{2})'(\d{2}(?:\.\d+)?)"\s*([US])\s+(\d{2,3})°(\d{2})'(\d{2}(?:\.\d+)?)"\s*([TEB])"#,
    ).unwrap()
}

fn parse_section_dc(text: &str) -> Vec<IslandRecord> {
    let mut results = Vec::new();
    let coord_re = coord_regex();

    let kab_header_re =
        regex::Regex::new(r"^\s*(\d{2}\.\d{2})\s+(Kabupaten|Kota)\s+(\S+(?:\s+\S+)*)\s+(\S+)\s*$")
            .unwrap();
    let island_code_re =
        regex::Regex::new(r"^\s*(\d{2}\.\d{2}\.\d{5})\s+(\S+(?:\s+\S+)*?)\s{2,}(.+)$").unwrap();
    let province_header_re = regex::Regex::new(r"^\s*D\.c\.\d+\)\s+Provinsi\s+(.+)$").unwrap();

    let mut current_kab_code = String::new();

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() || !coord_re.is_match(trimmed) {
            if province_header_re.captures(line).is_some() {
                // just consume
            }
            if let Some(cap) = kab_header_re.captures(line) {
                current_kab_code = cap[1].to_string();
            }
            // Also check for province-level kab headers like:
            // "  96             Papua Barat Daya                                     1"
            // Or:  "11.01           Kabupaten Aceh Selatan                              6"
            // The kab_header_re above already handles XX.XX code format
            continue;
        }

        if let Some(cap) = island_code_re.captures(line) {
            let code = cap[1].to_string();
            let name = cap[2].trim().to_string();
            let tail = cap[3].trim();
            let (latitude, longitude, area_km2, status, keterangan) =
                parse_island_tail(tail, &coord_re);
            results.push(IslandRecord {
                code,
                name,
                kabupaten_code: current_kab_code.clone(),
                latitude,
                longitude,
                area_km2,
                status,
                keterangan,
            });
            continue;
        }

        // Codeless island lines: start with many spaces, then island name, then coordinates
        // Must NOT start with a number (those are kab headers or coded islands)
        if !trimmed.starts_with(|c: char| c.is_ascii_digit()) {
            let name_and_tail = trimmed.trim();
            if let Some(m) = coord_re.captures(name_and_tail) {
                let name_end = m.get(0).unwrap().start();
                let name = name_and_tail[..name_end].trim().to_string();
                let tail = name_and_tail[m.get(0).unwrap().start()..].trim();
                let (latitude, longitude, area_km2, status, keterangan) =
                    parse_island_tail(tail, &coord_re);
                let synthetic_code = format!("{}.XXXXX", current_kab_code);
                results.push(IslandRecord {
                    code: synthetic_code,
                    name,
                    kabupaten_code: current_kab_code.clone(),
                    latitude,
                    longitude,
                    area_km2,
                    status,
                    keterangan,
                });
            }
        }
    }

    results
}

type IslandTail = (
    Option<String>,
    Option<String>,
    Option<f64>,
    Option<String>,
    Option<String>,
);

fn parse_island_tail(tail: &str, coord_re: &regex::Regex) -> IslandTail {
    if let Some(m) = coord_re.captures(tail) {
        let lat_deg: i32 = m[1].parse().unwrap_or(0);
        let lat_min: i32 = m[2].parse().unwrap_or(0);
        let lat_sec: f64 = m[3].parse().unwrap_or(0.0);
        let lat_dir = &m[4];
        let lon_deg: i32 = m[5].parse().unwrap_or(0);
        let lon_min: i32 = m[6].parse().unwrap_or(0);
        let lon_sec: f64 = m[7].parse().unwrap_or(0.0);
        let lon_dir = &m[8];

        let lat_decimal = if lat_dir == "S" {
            -(lat_deg as f64) - (lat_min as f64) / 60.0 - lat_sec / 3600.0
        } else {
            (lat_deg as f64) + (lat_min as f64) / 60.0 + lat_sec / 3600.0
        };
        let lon_decimal = if lon_dir == "T" || lon_dir == "E" {
            (lon_deg as f64) + (lon_min as f64) / 60.0 + lon_sec / 3600.0
        } else {
            -(lon_deg as f64) - (lon_min as f64) / 60.0 - lon_sec / 3600.0
        };

        let lat_str = format!("{:.6}", lat_decimal);
        let lon_str = format!("{:.6}", lon_decimal);

        let after_coord = tail[m.get(0).unwrap().end()..].trim();

        let (area_km2, status, keterangan) = parse_island_fields(after_coord);

        (Some(lat_str), Some(lon_str), area_km2, status, keterangan)
    } else {
        (None, None, None, None, None)
    }
}

fn parse_island_fields(mut rest: &str) -> (Option<f64>, Option<String>, Option<String>) {
    rest = rest.trim();

    if rest.is_empty() {
        return (None, None, None);
    }

    let mut area_km2: Option<f64> = None;
    let mut status: Option<String> = None;
    let mut keterangan: Option<String> = None;

    // Try to extract BP/TBP status first
    if rest.starts_with("BP") || rest.starts_with("TBP") {
        if rest.starts_with("TBP") {
            status = Some("TBP".to_string());
            rest = rest[3..].trim();
        } else if rest.starts_with("BP") {
            status = Some("BP".to_string());
            rest = rest[2..].trim();
        }
    } else {
        let tokens: Vec<&str> = rest.split_whitespace().collect();
        if !tokens.is_empty() {
            if let Ok(area) = tokens[0].parse::<f64>() {
                area_km2 = Some(area);
                rest = if tokens.len() > 1 {
                    rest[tokens[0].len()..].trim()
                } else {
                    ""
                };
                // Now check for status after area
                if rest.starts_with("BP") || rest.starts_with("TBP") {
                    if rest.starts_with("TBP") {
                        status = Some("TBP".to_string());
                        rest = rest[3..].trim();
                    } else if rest.starts_with("BP") {
                        status = Some("BP".to_string());
                        rest = rest[2..].trim();
                    }
                }
            }
        }
    }

    // Remaining text is keterangan
    if !rest.is_empty() {
        // Strip leading parentheses and trailing parentheses if present
        let ket = rest.trim();
        keterangan = Some(ket.to_string());
    }

    (area_km2, status, keterangan)
}

pub(crate) fn extract_islands(text: &str) -> (Vec<IslandSummary>, Vec<IslandRecord>) {
    let summaries = parse_section_db(text);
    let islands = parse_section_dc(text);
    (summaries, islands)
}

/// Save parsed island summary records to a JSON file.
pub(crate) fn save_parsed_island_summaries(
    summaries: &[IslandSummary],
    path: &Path,
) -> Result<(), super::PipelineError> {
    use super::PipelineResultExt;
    let json_str = serde_json::to_string_pretty(summaries)
        .ctx("failed to serialize parsed island summaries")?;
    std::fs::write(path, json_str).ctx("failed to write parsed island summaries JSON")?;
    eprintln!(
        "Saved {} parsed island summaries to {:?}",
        summaries.len(),
        path
    );
    Ok(())
}

/// Save parsed island detail records to a JSON file.
pub(crate) fn save_parsed_islands(
    islands: &[IslandRecord],
    path: &Path,
) -> Result<(), super::PipelineError> {
    use super::PipelineResultExt;
    let json_str =
        serde_json::to_string_pretty(islands).ctx("failed to serialize parsed islands")?;
    std::fs::write(path, json_str).ctx("failed to write parsed islands JSON")?;
    eprintln!("Saved {} parsed islands to {:?}", islands.len(), path);
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
            kel_count: None,
            desa_count: None,
            keterangan: None,
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
            kel_count: Some(0),
            desa_count: Some(7),
            keterangan: Some("Semula wil Kec. Bakongan".to_string()),
        }];
        let dir = std::env::temp_dir().join("wilayah_test_parse_raw");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("parsed.json");
        save_parsed_villages(&villages, ParseOutputDetail::WithRawName, &path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed[0]["raw_name"], "RAMBONG Semula wil Kec. Bakongan");
        assert!(parsed[0].get("note_keyword").is_none());
        assert_eq!(parsed[0]["kel_count"], 0);
        assert_eq!(parsed[0]["desa_count"], 7);
        assert_eq!(parsed[0]["keterangan"], "Semula wil Kec. Bakongan");
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
            kel_count: Some(0),
            desa_count: Some(7),
            keterangan: Some("Semula wil Kec. Bakongan".to_string()),
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
        assert_eq!(parsed[0]["kel_count"], 0);
        assert_eq!(parsed[0]["desa_count"], 7);
        assert_eq!(parsed[0]["keterangan"], "Semula wil Kec. Bakongan");
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
    fn test_find_column_gap_with_gap() {
        assert_eq!(
            find_column_gap("Ujong Mangki                             Perbaikan nama"),
            Some(12)
        );
    }

    #[test]
    fn test_find_column_gap_no_gap() {
        assert_eq!(find_column_gap("Keude Bakongan"), None);
    }

    #[test]
    fn test_find_column_gap_two_spaces_not_gap() {
        assert_eq!(find_column_gap("Hello  World"), None);
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

    #[test]
    fn test_strip_trailing_period_long_word() {
        let mut s = String::from("Air Sialang Hilir.");
        strip_trailing_period(&mut s);
        assert_eq!(s, "Air Sialang Hilir");
    }

    #[test]
    fn test_strip_trailing_period_four_char_word() {
        let mut s = String::from("U Baro.");
        strip_trailing_period(&mut s);
        assert_eq!(s, "U Baro");
    }

    #[test]
    fn test_strip_trailing_period_preserves_short_abbreviation() {
        let mut s = String::from("Papuyu I Sei.");
        strip_trailing_period(&mut s);
        assert_eq!(s, "Papuyu I Sei.");
    }

    #[test]
    fn test_strip_trailing_period_preserves_single_char() {
        let mut s = String::from("Pardomuan J.");
        strip_trailing_period(&mut s);
        assert_eq!(s, "Pardomuan J.");
    }

    #[test]
    fn test_strip_trailing_period_preserves_two_char() {
        let mut s = String::from("Bedeng SS.");
        strip_trailing_period(&mut s);
        assert_eq!(s, "Bedeng SS.");
    }

    #[test]
    fn test_strip_trailing_period_no_period() {
        let mut s = String::from("Suka Maju");
        strip_trailing_period(&mut s);
        assert_eq!(s, "Suka Maju");
    }

    #[test]
    fn test_capitalize_all_lowercase_single_word() {
        let mut s = String::from("lamuk");
        capitalize_all_lowercase(&mut s);
        assert_eq!(s, "Lamuk");
    }

    #[test]
    fn test_capitalize_all_lowercase_multi_word() {
        let mut s = String::from("suka maju");
        capitalize_all_lowercase(&mut s);
        assert_eq!(s, "Suka Maju");
    }

    #[test]
    fn test_capitalize_all_lowercase_preserves_mixed_case() {
        let mut s = String::from("Suka Maju");
        capitalize_all_lowercase(&mut s);
        assert_eq!(s, "Suka Maju");
    }

    #[test]
    fn test_capitalize_all_lowercase_preserves_uppercase() {
        let mut s = String::from("ABADIJAYA");
        capitalize_all_lowercase(&mut s);
        assert_eq!(s, "ABADIJAYA");
    }

    #[test]
    fn test_capitalize_all_lowercase_empty() {
        let mut s = String::from("");
        capitalize_all_lowercase(&mut s);
        assert_eq!(s, "");
    }

    #[test]
    fn test_district_name_banyuurip_fmt2() {
        let result = extract_district_name("33.06.07                                        7 Banyuurip                             3                           24");
        assert_eq!(result.name, "Banyuurip");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_district_name_mappakasunggu_fmt1() {
        let result = extract_district_name("     73.05.01        1 Mappakasunggu                                                      1         3");
        assert_eq!(result.name, "Mappakasunggu");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_district_name_mappakasunggu_fmt2() {
        let result = extract_district_name("73.05.01                                                     1 Mappakasunggu                                  1                        3");
        assert_eq!(result.name, "Mappakasunggu");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_district_name_proppo_fmt2() {
        let result = extract_district_name("35.28.05                                        5 Proppo                        -                          27");
        assert_eq!(result.name, "Proppo");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_district_name_puuwatu_fmt2() {
        let result = extract_district_name("74.71.09                                        9 Puuwatu                                6          -");
        assert_eq!(result.name, "Puuwatu");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_district_name_lalembuu() {
        let result = extract_district_name(
            "74.03.02  1 Lalembuu                        18                        -",
        );
        assert_eq!(result.name, "Lalembuu");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_district_name_batulappa() {
        let result = extract_district_name(
            "73.11.05  5 Batulappa                        5                        -",
        );
        assert_eq!(result.name, "Batulappa");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_district_name_soppeng_riaja() {
        let result = extract_district_name(
            "73.09.05  7 Soppeng Riaja                        7                        -",
        );
        assert_eq!(result.name, "Soppeng Riaja");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_district_name_suluun_tareran() {
        let result = extract_district_name(
            "71.05.09  9 Suluun Tareran                        9                        -",
        );
        assert_eq!(result.name, "Suluun Tareran");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_district_name_baruppu() {
        let result = extract_district_name(
            "73.21.04  4 Baruppu                        4                        -",
        );
        assert_eq!(result.name, "Baruppu");
        assert!(result.note.is_none());
    }

    #[test]
    fn test_strip_district_note_word_boundary() {
        let (name, note) = strip_district_note("Banyuurip");
        assert_eq!(name, "Banyuurip");
        assert!(note.is_none());

        let (name, note) = strip_district_note("Mappakasunggu");
        assert_eq!(name, "Mappakasunggu");
        assert!(note.is_none());

        let (name, note) = strip_district_note("Abenaho Semula wil Prov.");
        assert_eq!(name, "Abenaho");
        assert_eq!(note.as_deref(), Some("Semula wil Prov."));

        let (name, note) = strip_district_note("Makbon Semula Berdasarkan PP 22");
        assert_eq!(name, "Makbon");
        assert!(note.is_some());
    }

    #[test]
    fn test_parse_suffix_counts_empty() {
        let (kel, desa, start) = parse_suffix_counts(&[]);
        assert_eq!(kel, None);
        assert_eq!(desa, None);
        assert_eq!(start, 0);
    }

    #[test]
    fn test_parse_suffix_counts_kabupaten() {
        let (kel, desa, start) = parse_suffix_counts(&["-", "7"]);
        assert_eq!(kel, Some(0));
        assert_eq!(desa, Some(7));
        assert_eq!(start, 2);
    }

    #[test]
    fn test_parse_suffix_counts_kota() {
        let (kel, desa, start) = parse_suffix_counts(&["6", "-"]);
        assert_eq!(kel, Some(6));
        assert_eq!(desa, Some(0));
        assert_eq!(start, 2);
    }

    #[test]
    fn test_parse_suffix_counts_both_nonzero() {
        let (kel, desa, start) = parse_suffix_counts(&["2", "14"]);
        assert_eq!(kel, Some(2));
        assert_eq!(desa, Some(14));
        assert_eq!(start, 2);
    }

    #[test]
    fn test_parse_suffix_counts_single_count() {
        let (kel, desa, start) = parse_suffix_counts(&["7"]);
        assert_eq!(kel, Some(7));
        assert_eq!(desa, None);
        assert_eq!(start, 1);
    }

    #[test]
    fn test_parse_suffix_counts_with_note() {
        let (kel, desa, start) = parse_suffix_counts(&["-", "14", "Semula", "wil."]);
        assert_eq!(kel, Some(0));
        assert_eq!(desa, Some(14));
        assert_eq!(start, 2);
    }

    #[test]
    fn test_parse_suffix_counts_non_numeric_prefix() {
        let (kel, desa, start) = parse_suffix_counts(&["Semula", "wil."]);
        assert_eq!(kel, None);
        assert_eq!(desa, None);
        assert_eq!(start, 0);
    }

    #[test]
    fn test_extract_district_name_kabupaten_counts() {
        let result = extract_district_name(
            "11.01.01 1 Bakongan                       -                             7",
        );
        assert_eq!(result.name, "Bakongan");
        assert_eq!(result.kel_count, Some(0));
        assert_eq!(result.desa_count, Some(7));
    }

    #[test]
    fn test_extract_district_name_kota_counts() {
        let result = extract_district_name(
            "31.71.01 1 Gambir                                       6           -",
        );
        assert_eq!(result.name, "Gambir");
        assert_eq!(result.kel_count, Some(6));
        assert_eq!(result.desa_count, Some(0));
    }

    #[test]
    fn test_extract_district_name_counts_with_note() {
        let result = extract_district_name(
            "96.01.01 1 Makbon                       -                            14    Semula wil. Provinsi Papua Barat",
        );
        assert_eq!(result.name, "Makbon");
        assert_eq!(result.kel_count, Some(0));
        assert_eq!(result.desa_count, Some(14));
        assert!(result.note.is_some());
    }

    #[test]
    fn test_is_keterangan_continuation_deep_indent() {
        let line = "                                                                     tgl 14 okt 2016 dan Rekomedasi Ditjen Bina Pemdes No. 146/3672/BPD";
        let result = is_keterangan_continuation(line);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            "tgl 14 okt 2016 dan Rekomedasi Ditjen Bina Pemdes No. 146/3672/BPD"
        );
    }

    #[test]
    fn test_is_keterangan_continuation_shallow_indent() {
        let line = "   not deep enough";
        assert!(is_keterangan_continuation(line).is_none());
    }

    #[test]
    fn test_is_keterangan_continuation_village_code() {
        let line =
            "                                                                     11.01.01.2003";
        assert!(is_keterangan_continuation(line).is_none());
    }

    #[test]
    fn test_is_keterangan_continuation_empty() {
        assert!(is_keterangan_continuation("").is_none());
        assert!(is_keterangan_continuation("                    ").is_none());
    }

    #[test]
    fn test_extract_village_name_keterangan() {
        let re = name_re();
        let result = extract_village_name(
            "   2   Ujong Mangki                             Perbaikan nama sesuai Surat Pemkab Aceh Selatan No.140/819/2016",
            re,
        );
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.name, "Ujong Mangki");
        assert_eq!(
            r.keterangan.as_deref(),
            Some("Perbaikan nama sesuai Surat Pemkab Aceh Selatan No.140/819/2016")
        );
    }

    #[test]
    fn test_extract_village_name_no_keterangan() {
        let re = name_re();
        let result = extract_village_name("   1   Keude Bakongan", re);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.name, "Keude Bakongan");
        assert!(r.keterangan.is_none());
    }

    #[test]
    fn test_extract_districts() {
        let villages = vec![
            VillageRecord {
                code: "11.01.01.2001".to_string(),
                name: "Keude Bakongan".to_string(),
                district: "Bakongan".to_string(),
                city: "Aceh Selatan".to_string(),
                province: "Aceh".to_string(),
                raw_name: None,
                note_keyword: None,
                note_boundary: None,
                district_note: None,
                kel_count: Some(0),
                desa_count: Some(7),
                keterangan: None,
            },
            VillageRecord {
                code: "11.01.01.2002".to_string(),
                name: "Ujong Mangki".to_string(),
                district: "Bakongan".to_string(),
                city: "Aceh Selatan".to_string(),
                province: "Aceh".to_string(),
                raw_name: None,
                note_keyword: None,
                note_boundary: None,
                district_note: None,
                kel_count: Some(0),
                desa_count: Some(7),
                keterangan: None,
            },
            VillageRecord {
                code: "11.01.02.2001".to_string(),
                name: "Fajar Harapan".to_string(),
                district: "Kluet Utara".to_string(),
                city: "Aceh Selatan".to_string(),
                province: "Aceh".to_string(),
                raw_name: None,
                note_keyword: None,
                note_boundary: None,
                district_note: None,
                kel_count: Some(0),
                desa_count: Some(21),
                keterangan: None,
            },
        ];
        let districts = extract_districts(&villages);
        assert_eq!(districts.len(), 2);
        assert_eq!(districts[0].code, "11.01.01");
        assert_eq!(districts[0].name, "Bakongan");
        assert_eq!(districts[0].kel_count, Some(0));
        assert_eq!(districts[0].desa_count, Some(7));
        assert_eq!(districts[1].code, "11.01.02");
        assert_eq!(districts[1].name, "Kluet Utara");
        assert_eq!(districts[1].desa_count, Some(21));
    }

    #[test]
    #[ignore]
    fn test_parse_real_pdf_functional() {
        let text = match std::fs::read_to_string("/tmp/wilayah/pdf_text.txt") {
            Ok(t) => t,
            Err(_) => return,
        };
        let villages = parse_villages(&text);

        let with_kel = villages.iter().filter(|v| v.kel_count.is_some()).count();
        let with_desa = villages.iter().filter(|v| v.desa_count.is_some()).count();
        let with_ket = villages.iter().filter(|v| v.keterangan.is_some()).count();
        let with_long_ket = villages
            .iter()
            .filter(|v| v.keterangan.as_ref().map_or(false, |k| k.len() > 80))
            .count();

        eprintln!(
            "Total: {}  kel: {}  desa: {}  ket: {}  long_ket: {}",
            villages.len(),
            with_kel,
            with_desa,
            with_ket,
            with_long_ket
        );

        assert_eq!(villages.len(), 83756, "village count should match");
        assert!(with_kel > 7000, "most districts should have kel_count");
        assert!(with_desa > 7000, "most districts should have desa_count");
        assert!(with_ket > 0, "some villages should have keterangan");
        assert!(
            with_long_ket > 0,
            "some multi-line keterangan should be accumulated"
        );

        let bakongan = villages.iter().find(|v| v.code == "11.01.01.2001").unwrap();
        assert_eq!(bakongan.kel_count, Some(0));
        assert_eq!(bakongan.desa_count, Some(7));

        let gambir = villages.iter().find(|v| v.code == "31.71.01.1001").unwrap();
        assert_eq!(gambir.kel_count, Some(6));
        assert_eq!(gambir.desa_count, Some(0));

        let districts = extract_districts(&villages);
        eprintln!("Districts: {}", districts.len());
        assert!(districts.len() > 7000, "should have thousands of districts");

        // Verify multi-line keterangan accumulation for Ujong Mangki
        let ujiong = villages.iter().find(|v| v.code == "11.01.01.2002").unwrap();
        eprintln!("Ujong Mangki keterangan: {:?}", ujiong.keterangan);
        assert!(
            ujiong.keterangan.is_some(),
            "Ujong Mangki should have keterangan"
        );
        let kt = ujiong.keterangan.as_ref().unwrap();
        assert!(
            kt.contains("tgl 14 okt 2016"),
            "should have continuation line accumulated: {}",
            kt
        );
        assert!(
            kt.contains("tgl 21 Juni 2017"),
            "should have second continuation line: {}",
            kt
        );
    }

    #[test]
    fn test_parse_indonesian_int() {
        assert_eq!(parse_indonesian_int("5.623.479"), Some(5623479));
        assert_eq!(parse_indonesian_int("290"), Some(290));
        assert_eq!(parse_indonesian_int("0"), Some(0));
        assert_eq!(parse_indonesian_int("2.028"), Some(2028));
        assert_eq!(parse_indonesian_int("1.470.518"), Some(1470518));
        assert_eq!(parse_indonesian_int("17.380"), Some(17380));
    }

    #[test]
    fn test_parse_indonesian_float() {
        assert_eq!(parse_indonesian_float("56.835,019"), Some(56835.019));
        assert_eq!(parse_indonesian_float("661,530"), Some(661.530));
        assert_eq!(parse_indonesian_float("37.053,331"), Some(37053.331));
        assert_eq!(parse_indonesian_float("8.170,375"), Some(8170.375));
        assert_eq!(parse_indonesian_float("147.018,063"), Some(147018.063));
    }

    #[test]
    fn test_recover_ocr_number() {
        assert_eq!(recover_ocr_number("292.552"), Some(292552));
        assert_eq!(recover_ocr_number("r42.968"), Some(142968));
        assert_eq!(recover_ocr_number("'t05.374"), Some(105374));
        assert_eq!(recover_ocr_number("723.511"), Some(723511));
        assert_eq!(recover_ocr_number(""), None);
    }

    #[test]
    fn test_parse_section_a_sample() {
        let text = "  1     11    Aceh                                                   18             5         290         0    6.500       56.835,019         5.623.479            365\n  2     12    Sumatera Utara                                         25             8         455       693    5.417       72.437,755        15.640.905            228\n";
        let provinces = parse_section_a(text);
        assert_eq!(provinces.len(), 2);
        assert_eq!(provinces[0].code, "11");
        assert_eq!(provinces[0].name, "Aceh");
        assert_eq!(provinces[0].kab_count, 18);
        assert_eq!(provinces[0].kota_count, 5);
        assert_eq!(provinces[0].kec_count, 290);
        assert_eq!(provinces[0].kel_count, 0);
        assert_eq!(provinces[0].desa_count, 6500);
        assert_eq!(provinces[0].luas_km2, Some(56835.019));
        assert_eq!(provinces[0].penduduk, Some(5623479));
        assert_eq!(provinces[0].island_count, Some(365));

        assert_eq!(provinces[1].code, "12");
        assert_eq!(provinces[1].name, "Sumatera Utara");
        assert_eq!(provinces[1].kel_count, 693);
        assert_eq!(provinces[1].desa_count, 5417);
    }

    #[test]
    fn test_parse_c_province_headers_roman() {
        let line = " I       11      Aceh                             Banda Aceh    18     5       290                6500     56.835     5.623.479   Undang-undang Nomor 11 Tahun 2006 tentang Pemerintahan Aceh\n";
        let headers = parse_c_province_headers(line);
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].code, "11");
        assert_eq!(headers[0].name, "Aceh");
        assert_eq!(headers[0].ibukota.as_deref(), Some("Banda Aceh"));
        assert!(headers[0].keterangan.is_some());
    }

    #[test]
    fn test_parse_c_province_headers_arabic() {
        let line = "        93      Papua Selatan                Kabupaten Merauke    4     0       82      13        677     117.859      562.220    Undang-Undang Nomor 14 Tahun 2022 tentang Provinsi Papua\n";
        let headers = parse_c_province_headers(line);
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].code, "93");
        assert_eq!(headers[0].name, "Papua Selatan");
        assert_eq!(headers[0].ibukota.as_deref(), Some("Kabupaten Merauke"));
    }

    #[test]
    fn test_parse_c_city_headers_basic() {
        let text = "       11.01     1      Kabupaten Aceh Selatan    Tapaktuan                    18       0         260      4.174      239.629     Perbaikan nama ibu kota semula Tapak Tuan menjadi Tapaktuan\n       11.02     2      Kabupaten Aceh Tenggara    Kutacane                    16           385      4.179,123         235.589\n";
        let cities = parse_c_city_headers(text);
        assert_eq!(cities.len(), 2);
        assert_eq!(cities[0].code, "11.01");
        assert_eq!(cities[0].name, "Kabupaten Aceh Selatan");
        assert_eq!(cities[0].ibukota.as_deref(), Some("Tapaktuan"));
        assert_eq!(cities[0].kec_count, 18);
        assert!(cities[0].keterangan.is_some());
    }

    #[test]
    fn test_parse_section_e_basic() {
        let text = "     11    Aceh                                                     2.815.060              2.808.419           5.623.479\n   11.01   Kabupaten Aceh Selatan                                      120.041              119.588             239.629\n";
        let entries = parse_section_e(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].code, "11");
        assert_eq!(entries[0].male, Some(2815060));
        assert_eq!(entries[0].female, Some(2808419));
        assert_eq!(entries[0].total, Some(5623479));
        assert_eq!(entries[1].code, "11.01");
        assert_eq!(entries[1].male, Some(120041));
    }

    #[test]
    #[ignore]
    fn test_parse_real_pdf_phase2_functional() {
        let text = match std::fs::read_to_string("/tmp/wilayah/pdf_text.txt") {
            Ok(t) => t,
            Err(_) => return,
        };

        let provinces = extract_provinces(&text);
        eprintln!("Provinces: {}", provinces.len());
        assert_eq!(provinces.len(), 38, "should have 38 provinces");

        let aceh = provinces.iter().find(|p| p.code == "11").unwrap();
        assert_eq!(aceh.name, "Aceh");
        assert_eq!(aceh.kab_count, 18);
        assert_eq!(aceh.kota_count, 5);
        assert_eq!(aceh.kec_count, 290);
        assert_eq!(aceh.desa_count, 6500);
        assert!(aceh.ibukota.is_some(), "Aceh should have ibukota");
        assert_eq!(aceh.ibukota.as_deref(), Some("Banda Aceh"));
        assert!(aceh.luas_km2.is_some());
        assert!(aceh.penduduk.is_some());
        assert!(aceh.island_count.is_some());
        assert!(aceh.population_male.is_some());
        assert!(aceh.population_female.is_some());

        let papua_selatan = provinces.iter().find(|p| p.code == "93").unwrap();
        assert_eq!(papua_selatan.name, "Papua Selatan");
        assert!(papua_selatan.ibukota.is_some());

        let cities = extract_cities(&text);
        eprintln!("Cities: {}", cities.len());
        assert!(cities.len() > 500, "should have 500+ cities");

        let aceh_selatan = cities.iter().find(|c| c.code == "11.01").unwrap();
        assert_eq!(aceh_selatan.name, "Kabupaten Aceh Selatan");
        assert!(aceh_selatan.ibukota.is_some());
        eprintln!("Aceh Selatan ibukota: {:?}", aceh_selatan.ibukota);
        assert_eq!(aceh_selatan.kec_count, 18);
        assert!(aceh_selatan.penduduk.is_some());
        assert!(aceh_selatan.population_male.is_some());
    }

    #[test]
    fn test_parse_section_db_basic() {
        let text = "\
D.b. Rekapitulasi Jumlah Pulau Per Kabupaten/Kota Per Provinsi Seluruh Indonesia
 D.b.1) Provinsi Aceh

                                                            JUMLAH
 NO    KODE                             NAMA                            KETERANGAN
                                                             PULAU
  1    11.01   Kabupaten Aceh Selatan                             6
  2    11.03   Kabupaten Aceh Timur                               8
  3    11.06   Kabupaten Aceh Besar                              42
                                      TOTAL                     365
";
        let summaries = parse_section_db(text);
        assert_eq!(summaries.len(), 3);
        assert_eq!(summaries[0].code, "11.01");
        assert_eq!(summaries[0].name, "Kabupaten Aceh Selatan");
        assert_eq!(summaries[0].island_count, 6);
        assert_eq!(summaries[0].province, "Aceh");
        assert_eq!(summaries[1].island_count, 8);
        assert_eq!(summaries[2].island_count, 42);
    }

    #[test]
    fn test_parse_section_db_skip_zero() {
        let text = "\
 D.b.1) Provinsi Test
  1    12.07   Kabupaten Deli Serdang                             0
  2    12.14   Kabupaten Nias Selatan                            87
";
        let summaries = parse_section_db(text);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].code, "12.14");
        assert_eq!(summaries[0].island_count, 87);
    }

    #[test]
    fn test_parse_section_dc_basic() {
        let text = "\
D.c. Rincian Kode dan Data Pulau Per Kabupaten/Kota Per Provinsi Seluruh Indonesia
D.c.1) Provinsi Aceh
 11.01           Kabupaten Aceh Selatan                              6
 11.01.40001      Pulau Batukapal                                              03°19'03.44\" U 097°07'41.73\" T   0.0006     TBP
 11.01.40002      Pulau Batutunggal                                            03°24'55.00\" U 097°04'21.00\" T   0.0078     TBP
 11.01.40004      Pulau Mangki                                                 02°54'25.11\" U 097°26'18.51\" T              TBP
 11.01.40006      Pulau Trumon                                                 02°48'34.67\" U 097°35'36.51\" T              TBP
 11.06.40017      Pulau Breueh                                                 05°37'28.89\" U 095°09'18.31\" T   27.2422     BP
 11.06.40018      Pulau Bukulah Utara                                          05°37'29.97\" U 095°03'04.38\" T    0.0019
 11.06.40034      Pulau Rusa                                    05°16'39.00\" U 095°12'20.00\" T    0.2744    TBP     (PPKT)
";
        let islands = parse_section_dc(text);
        assert!(
            islands.len() >= 6,
            "expected at least 6 islands, got {}",
            islands.len()
        );

        let batukapal = islands.iter().find(|i| i.code == "11.01.40001").unwrap();
        assert_eq!(batukapal.name, "Pulau Batukapal");
        assert_eq!(batukapal.kabupaten_code, "11.01");
        assert!(batukapal.latitude.is_some());
        assert!(batukapal.longitude.is_some());
        assert!(batukapal.area_km2.is_some());
        assert_eq!(batukapal.status.as_deref(), Some("TBP"));
        assert!(batukapal.keterangan.is_none());

        let breueh = islands.iter().find(|i| i.code == "11.06.40017").unwrap();
        assert_eq!(breueh.name, "Pulau Breueh");
        assert_eq!(breueh.status.as_deref(), Some("BP"));
        assert!(breueh.area_km2.is_some());
        assert!((breueh.area_km2.unwrap() - 27.2422).abs() < 0.01);

        let bukulah = islands.iter().find(|i| i.code == "11.06.40018").unwrap();
        assert_eq!(bukulah.name, "Pulau Bukulah Utara");
        assert!(bukulah.area_km2.is_some());
        assert!(bukulah.status.is_none());

        let rusa = islands.iter().find(|i| i.code == "11.06.40034").unwrap();
        assert_eq!(rusa.status.as_deref(), Some("TBP"));
        assert_eq!(rusa.keterangan.as_deref(), Some("(PPKT)"));
    }

    #[test]
    fn test_parse_section_dc_southern_hemisphere() {
        let text = "\
 96.01           Kabupaten Sorong                                   122
 96.01.40063      Pulau Mokon                                   01°10'13.01\" S 130°37'37.80\" T   0.0582     TBP
";
        let islands = parse_section_dc(text);
        assert_eq!(islands.len(), 1);
        let mokon = &islands[0];
        assert_eq!(mokon.code, "96.01.40063");
        let lat: f64 = mokon.latitude.as_ref().unwrap().parse().unwrap();
        assert!(
            lat < 0.0,
            "Southern hemisphere latitude should be negative, got {}",
            lat
        );
    }

    #[test]
    fn test_parse_island_tail_with_area_and_status() {
        let coord_re = coord_regex();
        let tail = "03°19'03.44\" U 097°07'41.73\" T   0.0006     TBP";
        let (lat, lon, area, status, ket) = parse_island_tail(tail, &coord_re);
        assert!(lat.is_some());
        assert!(lon.is_some());
        assert!(area.is_some());
        assert_eq!(status.as_deref(), Some("TBP"));
        assert!(ket.is_none());
    }

    #[test]
    fn test_parse_island_tail_no_area() {
        let coord_re = coord_regex();
        let tail = "02°54'25.11\" U 097°26'18.51\" T              TBP";
        let (lat, lon, area, status, ket) = parse_island_tail(tail, &coord_re);
        assert!(lat.is_some());
        assert!(lon.is_some());
        assert!(area.is_none());
        assert_eq!(status.as_deref(), Some("TBP"));
        assert!(ket.is_none());
    }

    #[test]
    fn test_parse_island_tail_with_keterangan() {
        let coord_re = coord_regex();
        let tail = "05°16'39.00\" U 095°12'20.00\" T    0.2744    TBP     (PPKT)";
        let (_lat, _lon, area, status, ket) = parse_island_tail(tail, &coord_re);
        assert!(area.is_some());
        assert_eq!(status.as_deref(), Some("TBP"));
        assert_eq!(ket.as_deref(), Some("(PPKT)"));
    }

    #[test]
    fn test_parse_island_tail_only_coords() {
        let coord_re = coord_regex();
        let tail = "01°26'12.52\" S 131°58'41.87\" T";
        let (lat, lon, area, status, ket) = parse_island_tail(tail, &coord_re);
        assert!(lat.is_some());
        assert!(lon.is_some());
        assert!(area.is_none());
        assert!(status.is_none());
        assert!(ket.is_none());
    }

    #[test]
    fn test_parse_island_fields_area_then_status() {
        let (area, status, ket) = parse_island_fields("0.0006     TBP");
        assert!(area.is_some());
        assert_eq!(status.as_deref(), Some("TBP"));
        assert!(ket.is_none());
    }

    #[test]
    fn test_parse_island_fields_status_only() {
        let (area, status, ket) = parse_island_fields("TBP");
        assert!(area.is_none());
        assert_eq!(status.as_deref(), Some("TBP"));
        assert!(ket.is_none());
    }

    #[test]
    fn test_parse_island_fields_status_then_keterangan() {
        let (area, status, ket) =
            parse_island_fields("TBP     Alokasi pulau pindah ke Kota Sibolga");
        assert!(area.is_none());
        assert_eq!(status.as_deref(), Some("TBP"));
        assert_eq!(ket.as_deref(), Some("Alokasi pulau pindah ke Kota Sibolga"));
    }

    #[test]
    fn test_parse_island_fields_empty() {
        let (area, status, ket) = parse_island_fields("");
        assert!(area.is_none());
        assert!(status.is_none());
        assert!(ket.is_none());
    }

    #[test]
    fn test_functional_island_parsing() {
        let text = std::fs::read_to_string("/tmp/wilayah/pdf_text.txt");
        if text.is_err() {
            eprintln!("Skipping functional test: /tmp/wilayah/pdf_text.txt not found");
            return;
        }
        let text = text.unwrap();

        let (summaries, islands) = extract_islands(&text);
        eprintln!("Island summaries: {}", summaries.len());
        eprintln!("Island records: {}", islands.len());

        assert!(
            summaries.len() > 250,
            "expected 250+ island summaries, got {}",
            summaries.len()
        );
        assert!(
            islands.len() > 15000,
            "expected 15000+ island records, got {}",
            islands.len()
        );

        let aceh_summaries: Vec<_> = summaries.iter().filter(|s| s.province == "Aceh").collect();
        assert!(
            aceh_summaries.len() > 10,
            "Aceh should have 10+ cities with islands"
        );
        let aceh_total: u32 = aceh_summaries.iter().map(|s| s.island_count).sum();
        assert_eq!(aceh_total, 365, "Aceh island total should be 365");

        let first_aceh = summaries.iter().find(|s| s.code == "11.01").unwrap();
        assert_eq!(first_aceh.name, "Kabupaten Aceh Selatan");
        assert_eq!(first_aceh.island_count, 6);

        let batukapal = islands.iter().find(|i| i.code == "11.01.40001");
        assert!(batukapal.is_some(), "Should find Pulau Batukapal");
        let b = batukapal.unwrap();
        assert_eq!(b.name, "Pulau Batukapal");
        assert_eq!(b.kabupaten_code, "11.01");
        assert!(b.latitude.is_some());
        assert_eq!(b.status.as_deref(), Some("TBP"));

        let breueh = islands.iter().find(|i| i.code == "11.06.40017");
        assert!(breueh.is_some(), "Should find Pulau Breueh");
        let br = breueh.unwrap();
        assert_eq!(br.status.as_deref(), Some("BP"));

        let southern = islands
            .iter()
            .filter(|i| i.latitude.as_ref().map_or(false, |l| l.starts_with('-')))
            .count();
        assert!(
            southern > 100,
            "Should have 100+ southern hemisphere islands, got {}",
            southern
        );
    }
}
