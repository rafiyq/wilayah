//! Build pipeline for constructing the `wilayah` location database.
//!
//! This module provides an end-to-end pipeline that:
//! 1. Downloads the official Kemendagri PDF listing all Indonesian villages
//! 2. Extracts and parses village records from the PDF text
//! 3. Fetches village polygon boundaries from the BIG ArcGIS API and computes centroids
//! 4. Merges the data, using kecamatan centroids as fallback for new villages
//! 5. Builds a SQLite database with RTree spatial index and FTS5 full-text search
//!
//! # Example
//!
//! ```no_run
//! use wilayah::pipeline::Pipeline;
//!
//! let output = Pipeline::new()
//!     .output(std::path::Path::new("data/locations.db"))
//!     .run()
//!     .expect("pipeline failed");
//!
//! println!("Built database with {} villages", output.village_count);
//! ```
//!
//! The pipeline is designed to be reproducible and transparent, sourcing data from
//! official government publications and APIs. The resulting database is embedded
//! into the `wilayah` crate at compile time via the build script.

use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

/// The government decree number and year that the Kemendagri PDF data is based on.
pub const DATA_DECREE: &str = "Kepmendagri No 300.2.2-2138 Tahun 2025";

const PDF_URL: &str =
    "https://drive.google.com/uc?export=download&id=1o_m621D00TtwCwQMLn8XUnV3nolamPDm";
const BIG_API_URL: &str =
    "https://geoservices.big.go.id/gis/rest/services/BAPANAS/Batas_Administrasi/MapServer/2/query";
const BIG_BATCH_SIZE: usize = 1000;

type VillageTuple = (String, String, String, String, String, f64, f64);

/// Error type returned when a pipeline step fails.
///
/// Contains a descriptive error message indicating what went wrong during
/// the pipeline execution (e.g., download failure, parsing error, database
/// creation failure).
#[derive(Debug)]
pub struct PipelineError(String);

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for PipelineError {}

/// Output of a successful pipeline run.
#[allow(dead_code)]
pub struct PipelineOutput {
    /// Path to the built SQLite database file.
    pub db_path: PathBuf,
    /// Number of villages in the database.
    pub village_count: usize,
    /// SHA-256 hash of the database file, in hexadecimal.
    pub sha256: String,
}

/// Builder for configuring and running the database build pipeline.
///
/// The pipeline fetches data from official sources (Kemendagri PDF and BIG ArcGIS API),
/// merges and validates it, then constructs a SQLite database with RTree and FTS5
/// indexes. The resulting database is used by the `wilayah` crate at compile time.
#[allow(dead_code)]
pub struct Pipeline {
    pdf_url: String,
    big_api_url: String,
    cache_dir: PathBuf,
    output: PathBuf,
    decree: String,
    force_refresh_big: bool,
}

#[allow(dead_code)]
impl Pipeline {
    /// Creates a new `Pipeline` with default configuration.
    ///
    /// Defaults:
    /// - PDF URL: Kemendagri official PDF from Google Drive
    /// - BIG API URL: `https://geoservices.big.go.id/...`
    /// - Cache directory: `data/cache` (relative to current working directory)
    /// - Output database: `data/locations.db`
    /// - Decree: `DATA_DECREE` constant
    /// - `force_refresh_big`: `false`
    ///
    /// To change any of these, use the builder methods (`pdf_url()`, `cache_dir()`,
    /// etc.) before calling `run()`.
    pub fn new() -> Self {
        Self {
            pdf_url: PDF_URL.to_string(),
            big_api_url: BIG_API_URL.to_string(),
            cache_dir: PathBuf::from("data/cache"),
            output: PathBuf::from("data/locations.db"),
            decree: DATA_DECREE.to_string(),
            force_refresh_big: false,
        }
    }

    /// Overrides the default Kemendagri PDF download URL.
    ///
    /// The URL should point to a PDF file containing the official village listing.
    /// The default is the Google Drive link used by the Ministry of Home Affairs.
    pub fn pdf_url(mut self, url: &str) -> Self {
        self.pdf_url = url.to_string();
        self
    }

    /// Overrides the default BIG (Badan Informasi Geospasial) ArcGIS API endpoint.
    ///
    /// The pipeline queries this service for village polygon boundaries and computes
    /// centroids. The default is the public BAPANAS service.
    pub fn big_api_url(mut self, url: &str) -> Self {
        self.big_api_url = url.to_string();
        self
    }

    /// Sets the directory where intermediate files are cached.
    ///
    /// This includes the downloaded PDF (`kemendagri.pdf`) and cached BIG data
    /// (`big_villages.json`). The directory is created if it does not exist.
    pub fn cache_dir(mut self, dir: &Path) -> Self {
        self.cache_dir = dir.to_path_buf();
        self
    }

    /// Sets the output path for the final SQLite database.
    ///
    /// This file will be overwritten if it already exists. The parent directory
    /// must be writable.
    pub fn output(mut self, path: &Path) -> Self {
        self.output = path.to_path_buf();
        self
    }

    /// Overrides the government decree string stored in the database metadata.
    ///
    /// This value is for informational purposes only and appears in `DataInfo`.
    /// The default is `DATA_DECREE`.
    pub fn decree(mut self, decree: &str) -> Self {
        self.decree = decree.to_string();
        self
    }

    /// Forces re-downloading BIG API data even if a cached copy exists.
    ///
    /// By default, the pipeline uses the cached `big_villages.json` if present.
    /// Set this to `true` to fetch fresh data from the API.
    pub fn force_refresh_big(mut self, yes: bool) -> Self {
        self.force_refresh_big = yes;
        self
    }

    /// Executes the full pipeline.
    ///
    /// Steps:
    /// 1. Ensure Kemendagri PDF is downloaded (cached if already present)
    /// 2. Extract text from PDF using `pdftotext`
    /// 3. Parse village records from the extracted text
    /// 4. Fetch BIG polygon data (cached or fresh with retries)
    /// 5. Merge villages with coordinates, using kecamatan centroids as fallback
    /// 6. Build the SQLite database with indexes and optimize
    /// 7. Compute SHA-256 of the final database
    ///
    /// Returns `PipelineOutput` on success, or `PipelineError` if any step fails.
    pub fn run(self) -> Result<PipelineOutput, PipelineError> {
        eprintln!("Starting pipeline...");

        let pdf_path = ensure_pdf(&self.pdf_url, &self.cache_dir)?;
        let text = extract_text(&pdf_path)?;
        let villages = parse_villages(&text);
        let big_data = fetch_big_data(&self.big_api_url, &self.cache_dir, self.force_refresh_big)?;
        let merged = merge_villages(&villages, &big_data);

        build_db(&merged, &self.output)?;

        // Compute SHA-256
        let sha256 = compute_sha256(&self.output)?;

        let village_count = merged.len();

        eprintln!("Pipeline completed successfully.");
        Ok(PipelineOutput {
            db_path: self.output,
            village_count,
            sha256,
        })
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

fn ensure_pdf(pdf_url: &str, cache_dir: &Path) -> Result<PathBuf, PipelineError> {
    fs::create_dir_all(cache_dir)
        .map_err(|e| PipelineError(format!("failed to create cache directory: {e}")))?;
    let pdf_path = cache_dir.join("kemendagri.pdf");

    if !pdf_path.exists() {
        eprintln!("Downloading Kemendagri PDF (57 MB)...");
        let bytes = download_with_sha256(pdf_url)?;
        fs::write(&pdf_path, bytes.data)
            .map_err(|e| PipelineError(format!("failed to write PDF: {e}")))?;
        eprintln!("PDF SHA-256: {}", bytes.sha256);
    }

    Ok(pdf_path)
}

fn extract_text(pdf_path: &Path) -> Result<String, PipelineError> {
    eprintln!("Extracting text from PDF...");
    let output = std::process::Command::new("pdftotext")
        .arg("-layout")
        .arg(pdf_path)
        .arg("-")
        .output()
        .map_err(|e| PipelineError(format!("pdftotext failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PipelineError(format!(
            "pdftotext exited with status {}: {}",
            output.status, stderr
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_villages(text: &str) -> Vec<VillageRecord> {
    eprintln!("Parsing village records...");

    let village_code_re = regex::Regex::new(r"^(\d{2}\.\d{2}\.\d{2}\.\d{4})\s").unwrap();
    let kecamatan_code_re =
        regex::Regex::new(r"^\s*(\d{2}\.\d{2}\.\d{2})\s+\d+\s+([A-Z])").unwrap();
    let name_re = regex::Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
    let section_header_re = regex::Regex::new(r"C\.\w+\.\d+\)\s+(.+)$").unwrap();

    let mut villages = Vec::new();
    let mut current_province = "";
    let mut current_city = "";
    let mut current_district_code = String::new();
    let mut current_district_name = String::new();

    for line in text.lines() {
        if let Some(header) = parse_section_header(line, &section_header_re) {
            current_province = header.province;
            current_city = header.city;
            current_district_code.clear();
            current_district_name.clear();
        }

        if let Some(cap) = kecamatan_code_re.captures(line) {
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

        if let Some(code) = village_code_re.captures(line).and_then(|c| c.get(1)) {
            let code_str = code.as_str().to_string();
            let district_code = code_str[..8].to_string();
            if district_code != current_district_code {
                current_district_code = district_code.clone();
            }

            let after_code = &line[code.end()..];
            if let Some(name) = extract_village_name(after_code, &name_re) {
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

fn extract_village_name(after_code: &str, name_re: &regex::Regex) -> Option<String> {
    const NOTE_KEYWORDS: &[&str] = &[
        "Perbaikan",
        "perbaikan",
        "Pemekaran",
        "pemekaran",
        "Menjadi",
        "menjadi",
        "Qonun",
        "qonun",
        "Koreksi",
        "koreksi",
        "Penggabungan",
        "penggabungan",
        "Pembentukan",
        "pembentukan",
        "Penetapan",
        "penetapan",
        "Perubahan",
        "perubahan",
        "Peningkatan",
        "peningkatan",
        "Pemecahan",
        "pemecahan",
        "Nagari hasil",
        " Hasil",
        " hasil",
    ];

    let cap = name_re.captures(after_code)?;
    let raw = cap.get(1)?.as_str().trim();
    if raw.is_empty() || raw.chars().next().map(|c| c.is_numeric()).unwrap_or(false) {
        return None;
    }

    let mut earliest = raw.len();
    for keyword in NOTE_KEYWORDS {
        if let Some(pos) = raw.find(keyword) {
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

fn parse_section_header<'a>(line: &'a str, re: &regex::Regex) -> Option<SectionHeader<'a>> {
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

struct VillageRecord {
    code: String,
    name: String,
    district: String,
    city: String,
    province: String,
}

struct SectionHeader<'a> {
    province: &'a str,
    city: &'a str,
}

struct BigRecord {
    code: String,
    name: String,
    district: String,
    city: String,
    province: String,
    lat: f64,
    lon: f64,
}

fn fetch_big_data(
    api_url: &str,
    cache_dir: &Path,
    force_refresh: bool,
) -> Result<Vec<BigRecord>, PipelineError> {
    let cache_path = cache_dir.join("big_villages.json");

    if !force_refresh && cache_path.exists() {
        let content = fs::read_to_string(&cache_path)
            .map_err(|e| PipelineError(format!("failed to read BIG cache: {e}")))?;
        let records: Vec<serde_json::Value> = serde_json::from_str(&content)
            .map_err(|e| PipelineError(format!("failed to parse BIG cache: {e}")))?;
        let mut result = Vec::with_capacity(records.len());
        for r in records {
            if let (Some(code), Some(lat), Some(lon)) = (
                r.get("code").and_then(|v| v.as_str()),
                r.get("lat").and_then(|v| v.as_f64()),
                r.get("lon").and_then(|v| v.as_f64()),
            ) {
                result.push(BigRecord {
                    code: code.to_string(),
                    name: r
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    district: r
                        .get("district")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    city: r
                        .get("city")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    province: r
                        .get("province")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    lat,
                    lon,
                });
            }
        }
        eprintln!("Loaded {} BIG village records from cache", result.len());
        return Ok(result);
    }

    eprintln!("Fetching BIG village data from ArcGIS API...");
    fs::create_dir_all(cache_dir)
        .map_err(|e| PipelineError(format!("failed to create cache directory: {e}")))?;

    let mut all_records = Vec::new();
    let mut offset = 0;
    let mut batch_num = 0;

    loop {
        batch_num += 1;
        let url = format!(
            "{}?where=KDEPUM+IS+NOT+NULL\
             &outFields=KDEPUM,WADMKD,WADMKC,WADMKK,WADMPR\
             &returnGeometry=true\
             &f=json\
             &resultRecordCount={}\
             &resultOffset={}",
            api_url, BIG_BATCH_SIZE, offset
        );

        if batch_num % 10 == 1 || batch_num <= 3 {
            eprintln!("Fetching BIG batch {} (offset={})...", batch_num, offset);
        }

        let resp = fetch_with_retry(&url, 3)?;
        let json: serde_json::Value = serde_json::from_str(&resp)
            .map_err(|e| PipelineError(format!("failed to parse BIG API response: {e}")))?;

        if let Some(error) = json.get("error") {
            return Err(PipelineError(format!("BIG API error: {}", error)));
        }

        let features = json
            .get("features")
            .and_then(|f| f.as_array())
            .ok_or_else(|| PipelineError("missing features in BIG response".to_string()))?;

        if features.is_empty() {
            break;
        }

        for feature in features {
            let attrs = feature
                .get("attributes")
                .ok_or_else(|| PipelineError("missing attributes".to_string()))?;
            let code = attrs
                .get("KDEPUM")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let name = attrs
                .get("WADMKD")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let district = attrs
                .get("WADMKC")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let city = attrs
                .get("WADMKK")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let province = attrs
                .get("WADMPR")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if let (Some(code), Some(name)) = (code, name) {
                let geometry = feature.get("geometry");
                let (lat, lon) = if let Some(geom) = geometry {
                    compute_centroid(geom)
                } else {
                    (0.0, 0.0)
                };

                all_records.push(BigRecord {
                    code,
                    name,
                    district: district.unwrap_or_default(),
                    city: city.unwrap_or_default(),
                    province: province.unwrap_or_default(),
                    lat,
                    lon,
                });
            }
        }

        if features.len() < BIG_BATCH_SIZE {
            break;
        }

        offset += BIG_BATCH_SIZE;
    }

    eprintln!(
        "Fetched {} BIG village records in {} batches",
        all_records.len(),
        batch_num
    );

    // Save cache
    let cache_data: Vec<serde_json::Value> = all_records
        .iter()
        .map(|r| {
            serde_json::json!({
                "code": r.code,
                "name": r.name,
                "district": r.district,
                "city": r.city,
                "province": r.province,
                "lat": r.lat,
                "lon": r.lon,
            })
        })
        .collect();
    let cache_json = serde_json::to_string(&cache_data)
        .map_err(|e| PipelineError(format!("failed to serialize BIG cache: {e}")))?;
    fs::write(&cache_path, cache_json)
        .map_err(|e| PipelineError(format!("failed to write BIG cache: {e}")))?;
    eprintln!("Saved BIG cache to {:?}", cache_path);

    Ok(all_records)
}

fn fetch_with_retry(url: &str, max_retries: usize) -> Result<String, PipelineError> {
    let mut last_err = String::new();
    for attempt in 0..=max_retries {
        match ureq::get(url)
            .timeout(std::time::Duration::from_secs(60))
            .call()
        {
            Ok(resp) => {
                let mut buf = String::new();
                resp.into_reader()
                    .read_to_string(&mut buf)
                    .map_err(|e| PipelineError(format!("failed to read response: {e}")))?;
                return Ok(buf);
            }
            Err(e) => {
                last_err = format!("{}", e);
                if attempt < max_retries {
                    let wait_secs = 2_u64.pow(attempt as u32);
                    eprintln!(
                        "BIG API attempt {} failed, retrying in {}s: {}",
                        attempt + 1,
                        wait_secs,
                        last_err
                    );
                    std::thread::sleep(std::time::Duration::from_secs(wait_secs));
                }
            }
        }
    }
    Err(PipelineError(format!(
        "BIG API failed after {} retries: {}",
        max_retries, last_err
    )))
}

fn compute_centroid(geometry: &serde_json::Value) -> (f64, f64) {
    let mut rings: Vec<&[serde_json::Value]> = Vec::new();

    if let Some(rings_array) = geometry.get("rings").and_then(|r| r.as_array()) {
        for ring_val in rings_array {
            if let Some(ring) = ring_val.as_array() {
                rings.push(ring);
            }
        }
    } else if let Some(coord_arrays) = geometry.get("coordinates").and_then(|c| c.as_array()) {
        if let Some(first) = coord_arrays.first() {
            if first.get(0).map(|r| r.is_array()).unwrap_or(false) {
                for poly in coord_arrays {
                    if let Some(poly_rings) = poly.as_array() {
                        if let Some(outer) = poly_rings.first() {
                            if let Some(outer_ring) = outer.as_array() {
                                rings.push(outer_ring);
                            }
                        }
                    }
                }
            } else {
                rings.push(coord_arrays);
            }
        }
    }

    if rings.is_empty() {
        return (0.0, 0.0);
    }

    let mut largest_ring = &rings[0];
    let mut max_len = 0;
    for ring in &rings {
        if ring.len() > max_len {
            max_len = ring.len();
            largest_ring = ring;
        }
    }

    polygon_centroid(largest_ring)
}

fn polygon_centroid(ring: &[serde_json::Value]) -> (f64, f64) {
    if ring.len() < 3 {
        return (0.0, 0.0);
    }

    let mut area = 0.0_f64;
    let mut cx = 0.0_f64;
    let mut cy = 0.0_f64;
    let n = ring.len();

    for i in 0..n {
        let j = (i + 1) % n;

        let x_i = ring[i].get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y_i = ring[i].get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let x_j = ring[j].get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y_j = ring[j].get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);

        let cross = x_i * y_j - x_j * y_i;
        area += cross;
        cx += (x_i + x_j) * cross;
        cy += (y_i + y_j) * cross;
    }

    area *= 0.5;
    if area.abs() < 1e-10 {
        let mut sx = 0.0_f64;
        let mut sy = 0.0_f64;
        for pt in ring {
            sx += pt.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
            sy += pt.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
        }
        return (sy / ring.len() as f64, sx / ring.len() as f64);
    }

    cx /= 6.0 * area;
    cy /= 6.0 * area;

    (cy, cx)
}

fn merge_villages(villages: &[VillageRecord], big_data: &[BigRecord]) -> Vec<VillageTuple> {
    let big_lookup: std::collections::HashMap<&str, &BigRecord> =
        big_data.iter().map(|r| (r.code.as_str(), r)).collect();

    let mut kecamatan_coords: std::collections::HashMap<String, Vec<(f64, f64)>> =
        std::collections::HashMap::new();
    for r in big_data {
        let kec_key = format!("{}|{}|{}", r.province, r.city, r.district);
        kecamatan_coords
            .entry(kec_key)
            .or_default()
            .push((r.lat, r.lon));
    }
    let kecamatan_centroids: std::collections::HashMap<String, (f64, f64)> = kecamatan_coords
        .into_iter()
        .map(|(key, coords)| {
            let avg_lat = coords.iter().map(|(lat, _)| lat).sum::<f64>() / coords.len() as f64;
            let avg_lon = coords.iter().map(|(_, lon)| lon).sum::<f64>() / coords.len() as f64;
            (key, (avg_lat, avg_lon))
        })
        .collect();

    let mut merged = Vec::with_capacity(villages.len());
    let mut matched = 0;
    let mut fallback = 0;

    for v in villages {
        if let Some(big) = big_lookup.get(v.code.as_str()) {
            merged.push((
                v.code.clone(),
                v.name.clone(),
                v.district.clone(),
                v.city.clone(),
                v.province.clone(),
                big.lat,
                big.lon,
            ));
            matched += 1;
        } else {
            let kec_key = format!("{}|{}|{}", v.province, v.city, v.district);
            let (lat, lon) = kecamatan_centroids
                .get(&kec_key)
                .copied()
                .unwrap_or((0.0, 0.0));
            merged.push((
                v.code.clone(),
                v.name.clone(),
                v.district.clone(),
                v.city.clone(),
                v.province.clone(),
                lat,
                lon,
            ));
            fallback += 1;
        }
    }

    eprintln!(
        "Merged {} villages: {} matched BIG, {} fallback to kecamatan centroid",
        matched + fallback,
        matched,
        fallback
    );
    merged
}

fn build_db(villages: &[VillageTuple], db_path: &Path) -> Result<(), PipelineError> {
    if db_path.exists() {
        fs::remove_file(db_path)
            .map_err(|e| PipelineError(format!("failed to remove existing DB: {e}")))?;
    }

    let mut conn = Connection::open(db_path)
        .map_err(|e| PipelineError(format!("failed to create DB: {e}")))?;
    conn.execute_batch(
        "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA page_size = 4096;",
    )
    .map_err(|e| PipelineError(format!("PRAGMA failed: {e}")))?;

    conn.execute(
        "CREATE TABLE locations (
            id INTEGER PRIMARY KEY, kode TEXT NOT NULL UNIQUE, nama TEXT NOT NULL,
            kecamatan TEXT NOT NULL, kota TEXT NOT NULL, provinsi TEXT NOT NULL,
            lat REAL NOT NULL, lon REAL NOT NULL
        )",
        [],
    )
    .map_err(|e| PipelineError(format!("failed to create locations table: {e}")))?;

    conn.execute(
        "CREATE VIRTUAL TABLE geo_rtree USING rtree(id, min_lon, max_lon, min_lat, max_lat)",
        [],
    )
    .map_err(|e| PipelineError(format!("failed to create RTree: {e}")))?;

    conn.execute(
        "CREATE VIRTUAL TABLE locations_fts USING fts5(
            nama, kecamatan, kota, provinsi, content='locations', content_rowid='id'
        )",
        [],
    )
    .map_err(|e| PipelineError(format!("failed to create FTS5: {e}")))?;

    conn.execute("CREATE INDEX idx_locations_nama ON locations(nama)", [])
        .map_err(|e| PipelineError(format!("failed to create nama index: {e}")))?;
    conn.execute(
        "CREATE UNIQUE INDEX idx_locations_kode ON locations(kode)",
        [],
    )
    .map_err(|e| PipelineError(format!("failed to create kode index: {e}")))?;

    let tx = conn
        .transaction()
        .map_err(|e| PipelineError(format!("failed to begin transaction: {e}")))?;
    {
        let mut ins_loc = tx.prepare(
            "INSERT INTO locations (id, kode, nama, kecamatan, kota, provinsi, lat, lon) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
        ).map_err(|e| PipelineError(format!("prepare insert locations: {e}")))?;
        let mut ins_rtree = tx.prepare(
            "INSERT INTO geo_rtree (id, min_lon, max_lon, min_lat, max_lat) VALUES (?1, ?2, ?3, ?4, ?5)"
        ).map_err(|e| PipelineError(format!("prepare insert rtree: {e}")))?;

        for (i, (kode, nama, kecamatan, kota, provinsi, lat, lon)) in villages.iter().enumerate() {
            let rowid = (i + 1) as i64;
            ins_loc
                .execute(rusqlite::params![
                    rowid, kode, nama, kecamatan, kota, provinsi, lat, lon
                ])
                .map_err(|e| PipelineError(format!("insert location: {e}")))?;
            ins_rtree
                .execute(rusqlite::params![rowid, lon, lon, lat, lat])
                .map_err(|e| PipelineError(format!("insert rtree: {e}")))?;
        }
    }
    tx.commit()
        .map_err(|e| PipelineError(format!("failed to commit transaction: {e}")))?;

    conn.execute(
        "INSERT INTO locations_fts(locations_fts) VALUES('rebuild')",
        [],
    )
    .map_err(|e| PipelineError(format!("failed to rebuild FTS5: {e}")))?;

    conn.execute_batch("PRAGMA analysis_limit = 400; PRAGMA optimize; VACUUM;")
        .map_err(|e| PipelineError(format!("optimize failed: {e}")))?;

    let size = fs::metadata(db_path)
        .map_err(|e| PipelineError(format!("failed to get DB metadata: {e}")))?;
    eprintln!(
        "Database written: {:.1} MB",
        size.len() as f64 / (1024.0 * 1024.0)
    );

    Ok(())
}

fn compute_sha256(db_path: &Path) -> Result<String, PipelineError> {
    let data = fs::read(db_path)
        .map_err(|e| PipelineError(format!("failed to read DB for SHA-256: {e}")))?;
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(&data);
    Ok(format!("{:x}", hasher.finalize()))
}

struct DownloadResult {
    data: Vec<u8>,
    sha256: String,
}

fn download_with_sha256(url: &str) -> Result<DownloadResult, PipelineError> {
    let max_retries = 5;
    let mut last_err = String::new();

    for attempt in 0..=max_retries {
        match ureq::get(url)
            .timeout(std::time::Duration::from_secs(300))
            .call()
        {
            Ok(resp) => {
                let mut reader = resp.into_reader();
                let mut data = Vec::new();
                reader
                    .read_to_end(&mut data)
                    .map_err(|e| PipelineError(format!("failed to read response: {e}")))?;

                use sha2::Digest;
                let mut hasher = sha2::Sha256::new();
                hasher.update(&data);
                let sha256 = format!("{:x}", hasher.finalize());

                return Ok(DownloadResult { data, sha256 });
            }
            Err(e) => {
                last_err = format!("{}", e);
                if attempt < max_retries {
                    let wait_secs = 2_u64.pow(attempt as u32);
                    eprintln!(
                        "PDF download attempt {} failed, retrying in {}s: {}",
                        attempt + 1,
                        wait_secs,
                        last_err
                    );
                    std::thread::sleep(std::time::Duration::from_secs(wait_secs));
                }
            }
        }
    }

    Err(PipelineError(format!(
        "Failed to download PDF after {} retries: {}\n\
         Hint: Manually download the PDF from:\n\
         https://drive.google.com/file/d/1o_m621D00TtwCwQMLn8XUnV3nolamPDm/view\n\
         and place it at data/cache/kemendagri.pdf, then re-run cargo build.",
        max_retries, last_err
    )))
}
