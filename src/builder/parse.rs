//! PDF text parsing for village records.

/// A parsed village record from the Kemendagri PDF.
pub(crate) struct VillageRecord {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) district: String,
    pub(crate) city: String,
    pub(crate) province: String,
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
    "Menjadi",
    "Berubah",
    "Penataan",
    "Pengkatan",
    "Penghapusan",
    "Lampiran",
    "Letak",
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
    "PMD",
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

/// Find the earliest note boundary in `raw` by checking all note keywords.
///
/// Self-validating keywords always mark a note boundary.
/// Reference-validated keywords only mark a boundary if followed by a
/// reference-like pattern within 30 characters.
fn find_note_boundary(raw: &str, raw_lower: &str) -> usize {
    let mut earliest = raw.len();

    for keyword in SELF_VALIDATING_KEYWORDS {
        let kw_lower = keyword.to_lowercase();
        if let Some(pos) = raw_lower.find(&kw_lower) {
            earliest = earliest.min(pos);
        }
    }

    for keyword in REFERENCE_VALIDATED_KEYWORDS {
        let kw_lower = keyword.to_lowercase();
        if let Some(pos) = raw_lower.find(&kw_lower) {
            if has_reference_indicator(raw_lower, pos + kw_lower.len(), 30) {
                earliest = earliest.min(pos);
            }
        }
    }

    earliest
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

        for line in text.lines() {
            if let Some(header) = parse_section_header(line, &self.section_header_re) {
                current_province = header.province;
                current_city = header.city;
                current_district_code.clear();
                current_district_name.clear();
            }

            if let Some(cap) = self.kecamatan_code_re.captures(line) {
                current_district_code = cap.get(1).unwrap().as_str().to_string();
                let after_prefix = &line[cap.get(0).unwrap().start()..];
                current_district_name = extract_district_name(after_prefix);
                continue;
            }

            if let Some(code) = self.village_code_re.captures(line).and_then(|c| c.get(1)) {
                let code_str = code.as_str().to_string();
                let district_code = code_str[..8].to_string();
                if district_code != current_district_code {
                    current_district_code = district_code.clone();
                }

                let after_code = &line[code.end()..];
                if let Some(name) = extract_village_name(after_code, &self.name_re) {
                    villages.push(VillageRecord {
                        code: code_str,
                        name,
                        district: if current_district_name.is_empty() {
                            current_district_code.clone()
                        } else {
                            current_district_name.clone()
                        },
                        city: current_city.to_string(),
                        province: current_province.to_string(),
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

/// Extract the district name from the text after a kecamatan code match.
///
/// Kecamatan lines have the format: `CODE NUMBER NAME - VILLAGE_COUNT`
/// e.g., `31.73.01 60 KECAMATAN BALEENDAH - 7`
///
/// This function:
/// 1. Finds the last digit in the line (the village count)
/// 2. Takes text before it, trims trailing dashes/commas/spaces
/// 3. Extracts the name portion (starting from first non-separator character
///    after the code+number prefix)
fn extract_district_name(after_prefix: &str) -> String {
    if let Some(name_end) = after_prefix.rfind(|c: char| c.is_ascii_digit()) {
        let name_part = after_prefix[..name_end].trim();
        let cleaned = strip_trailing_separators(name_part);
        let name = skip_code_prefix(cleaned);
        return strip_trailing_separators(name).to_string();
    }
    String::new()
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

/// Strip trailing dashes, commas, and spaces from a string, iteratively.
///
/// Handles patterns like `" - "`, `" -"`, `", "`, `" ,"` etc.
fn strip_trailing_separators(s: &str) -> &str {
    let mut result = s;
    loop {
        let trimmed = result.trim_end_matches(['-', ',', ' ']);
        if trimmed.len() == result.len() {
            break;
        }
        result = trimmed;
    }
    result
}

/// Extract a village name from the text after the village code, stripping notes.
pub(crate) fn extract_village_name(after_code: &str, name_re: &regex::Regex) -> Option<String> {
    let cap = name_re.captures(after_code)?;
    let raw = cap.get(1)?.as_str().trim();
    if raw.is_empty() || raw.chars().next().map(|c| c.is_numeric()).unwrap_or(false) {
        return None;
    }

    let raw_lower = raw.to_lowercase();
    let boundary = find_note_boundary(raw, &raw_lower);
    let name = raw[..boundary].trim();
    if name.is_empty() {
        None
    } else {
        Some(
            name.split_whitespace()
                .take(MAX_NAME_WORDS)
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn test_parse_section_header_with_city_and_province() {
        let re = Regex::new(r"C\.\w+\.\d+\)\s+(.+)$").unwrap();
        let line = "C.Kabupaten.1) Kabupaten Bogor Provinsi Jawa Barat";
        let header = parse_section_header(line, &re);
        assert!(header.is_some());
        let h = header.unwrap();
        assert_eq!(h.province, "Provinsi Jawa Barat");
        assert_eq!(h.city, "Kabupaten Bogor");
    }

    #[test]
    fn test_parse_section_header_province_only() {
        let re = Regex::new(r"C\.\w+\.\d+\)\s+(.+)$").unwrap();
        let line = "C.Provinsi.1) Provinsi DKI Jakarta";
        let header = parse_section_header(line, &re);
        assert!(header.is_some());
        let h = header.unwrap();
        assert_eq!(h.province, "Provinsi DKI Jakarta");
        assert_eq!(h.city, "");
    }

    #[test]
    fn test_parse_section_header_no_provinsi() {
        let re = Regex::new(r"C\.\w+\.\d+\)\s+(.+)$").unwrap();
        let line = "C.Kabupaten.1) Some text without Provinsi";
        assert!(parse_section_header(line, &re).is_none());
    }

    #[test]
    fn test_parse_section_header_no_match() {
        let re = Regex::new(r"C\.\w+\.\d+\)\s+(.+)$").unwrap();
        let line = "31.12.24.2002 ABADMULIA KEC. BUKIT SARI";
        assert!(parse_section_header(line, &re).is_none());
    }

    #[test]
    fn test_parse_section_header_rejects_invalid_format() {
        let re = Regex::new(r"C\.\w+\.\d+\)\s+(.+)$").unwrap();
        let line = "C.Kabupaten.X) Kabupaten Bogor Provinsi Jawa Barat";
        assert!(parse_section_header(line, &re).is_none());
    }

    #[test]
    fn test_extract_village_name_basic() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 12 ABADIJAYA";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("ABADIJAYA".to_string()));
    }

    #[test]
    fn test_extract_village_name_multi_word() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 12 SUKA MAJU";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("SUKA MAJU".to_string()));
    }

    #[test]
    fn test_extract_village_name_keyword_stripping() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 15 SUKAMAJU KEMENANGAN Pemekaran menjadi SUKAMAJU";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("SUKAMAJU KEMENANGAN".to_string()));
    }

    #[test]
    fn test_extract_village_name_numeric_start() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 20 5SAFARI Some text";
        let name = extract_village_name(after_code, &name_re);
        assert!(name.is_none());
    }

    #[test]
    fn test_extract_village_name_empty() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 30 ";
        let name = extract_village_name(after_code, &name_re);
        assert!(name.is_none());
    }

    #[test]
    fn test_extract_village_name_truncate_to_five_words() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 10 DESA SUKAMAJU KECAMATAN BUKIT SARI LAINNYA";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("DESA SUKAMAJU KECAMATAN BUKIT SARI".to_string()));
    }

    #[test]
    fn test_extract_village_name_six_words_truncated() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 10 A B C D E F";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("A B C D E".to_string()));
    }

    #[test]
    fn test_extract_village_name_semula() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 2 RAMBONG Semula wil Kec. Bakongan Perda No. 3/2010";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("RAMBONG".to_string()));
    }

    #[test]
    fn test_extract_village_name_qanun() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 23 PIRAK TIMU Qanun No. 32/2005";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("PIRAK TIMU".to_string()));
    }

    #[test]
    fn test_extract_village_name_uu_with_reference() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 5 SUKAMAKMUR UU No. 4/2002";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("SUKAMAKMUR".to_string()));
    }

    #[test]
    fn test_extract_village_name_uu_without_reference_not_stripped() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 5 UU JAYA";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("UU JAYA".to_string()));
    }

    #[test]
    fn test_extract_village_name_hasil_in_name_not_stripped() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 18 HASIL JAYA";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("HASIL JAYA".to_string()));
    }

    #[test]
    fn test_extract_village_name_hal_hasil_with_reference() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 18 LIYA BAHARI Hal Hasil Klarifikasi Nama Desa";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("LIYA BAHARI".to_string()));
    }

    #[test]
    fn test_extract_village_name_amar_with_reference() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 12 MERDEKA Amar Putusan Mahkamah Agung RI Nomor 395K";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("MERDEKA".to_string()));
    }

    #[test]
    fn test_extract_village_name_perda_with_number() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 9 LEUBOK PASI Perda No. 3/2010 tentang pemekaran";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("LEUBOK PASI".to_string()));
    }

    #[test]
    fn test_extract_village_name_five_word_name_preserved() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 7 TANAH SIRAH PIAI NAN XX";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("TANAH SIRAH PIAI NAN XX".to_string()));
    }

    #[test]
    fn test_extract_village_name_perbaikan_with_nama() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 2 UJONG MANGKI Perbaikan nama sesuai Surat Pemkab";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("UJONG MANGKI".to_string()));
    }

    #[test]
    fn test_extract_village_name_nd_with_reference() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 5 SUKAJADI ND Rekom No 140/4495/BPD";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("SUKAJADI".to_string()));
    }

    #[test]
    fn test_parse_kecamatan_digit_name() {
        let text = "\
C.Kabupaten.1) Kabupaten Pasaman Barat Provinsi Sumatera Barat
13.05.04 4 2 x 11 Anam Lingkuang 3
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
        assert_eq!(
            extract_district_name("31.73.01 60 KECAMATAN BALEENDAH - 7"),
            "KECAMATAN BALEENDAH"
        );
    }

    #[test]
    fn test_extract_district_name_trailing_dash_no_space() {
        assert_eq!(
            extract_district_name("31.73.01 60 KECAMATAN BALEENDAH-7"),
            "KECAMATAN BALEENDAH"
        );
    }

    #[test]
    fn test_extract_district_name_no_trailing_count() {
        assert_eq!(
            extract_district_name("31.73.01 60 KECAMATAN BALEENDAH 7"),
            "KECAMATAN BALEENDAH"
        );
    }

    #[test]
    fn test_strip_trailing_separators() {
        assert_eq!(strip_trailing_separators("hello - "), "hello");
        assert_eq!(strip_trailing_separators("hello-"), "hello");
        assert_eq!(strip_trailing_separators("hello ,"), "hello");
        assert_eq!(strip_trailing_separators("hello -  "), "hello");
        assert_eq!(strip_trailing_separators("hello"), "hello");
        assert_eq!(strip_trailing_separators("hello - - "), "hello");
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
        assert_eq!(find_note_boundary(raw, &raw_lower), 8);
    }

    #[test]
    fn test_find_note_boundary_with_reference() {
        let raw = "SUKAMAKMUR UU No. 4/2002";
        let raw_lower = raw.to_lowercase();
        assert_eq!(find_note_boundary(raw, &raw_lower), 11);
    }

    #[test]
    fn test_find_note_boundary_without_reference() {
        let raw = "UU JAYA";
        let raw_lower = raw.to_lowercase();
        assert_eq!(find_note_boundary(raw, &raw_lower), raw.len());
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
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 5 CIPAGARANTU";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("CIPAGARANTU".to_string()));
    }
}
