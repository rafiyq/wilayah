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
            village_code_re: regex::Regex::new(r"^(\d{2}\.\d{2}\.\d{2}\.\d{4})\s").unwrap(),
            kecamatan_code_re: regex::Regex::new(r"^\s*(\d{2}\.\d{2}\.\d{2})\s+\d+\s+([A-Z])")
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
                if let Some(name_end) = after_prefix.rfind(|c: char| c.is_ascii_digit()) {
                    let name_part = after_prefix[..name_end].trim();
                    if let Some(name_start) = name_part.find(|c: char| c.is_ascii_alphabetic()) {
                        current_district_name = name_part[name_start..].trim().to_string();
                    }
                }
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

/// Extract a village name from the text after the village code, stripping notes.
pub(crate) fn extract_village_name(after_code: &str, name_re: &regex::Regex) -> Option<String> {
    const NOTE_KEYWORDS: &[&str] = &[
        "Perbaikan",
        "Pemekaran",
        "Menjadi",
        "Qonun",
        "Koreksi",
        "Penggabungan",
        "Pembentukan",
        "Penetapan",
        "Perubahan",
        "Peningkatan",
        "Pemecahan",
        "Nagari hasil",
        "Hasil",
    ];

    let cap = name_re.captures(after_code)?;
    let raw = cap.get(1)?.as_str().trim();
    if raw.is_empty() || raw.chars().next().map(|c| c.is_numeric()).unwrap_or(false) {
        return None;
    }

    let mut earliest = raw.len();
    for keyword in NOTE_KEYWORDS {
        if let Some(pos) = raw.to_lowercase().find(&keyword.to_lowercase()) {
            earliest = earliest.min(pos);
        }
    }
    let name = raw[..earliest].trim();
    if name.is_empty() {
        None
    } else {
        Some(
            name.split_whitespace()
                .take(4)
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
    fn test_extract_village_name_truncate_to_four_words() {
        let name_re = Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
        let after_code = " 10 DESA SUKAMAJU KECAMATAN BUKIT SARI LAINNYA";
        let name = extract_village_name(after_code, &name_re);
        assert_eq!(name, Some("DESA SUKAMAJU KECAMATAN BUKIT".to_string()));
    }

    #[test]
    fn test_parse_villages_basic() {
        let text = "\
C.Kabupaten.1) Kabupaten Bandung Provinsi Jawa Barat
31.73.01 60 KECAMATAN BALEENDAH
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
C.Kabupaten.1) Kabupaten Bandung Provinsi Jawa Barat
31.73.01 60 KECAMATAN BALEENDAH
31.73.01.1001 5 CIPAGARANTU
31.73.01.1002 12 MARGASARI";
        let villages = parser.parse(text);
        assert_eq!(villages.len(), 2);
        assert_eq!(villages[0].code, "31.73.01.1001");
        assert_eq!(villages[1].code, "31.73.01.1002");
    }
}
