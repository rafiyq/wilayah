use std::fs;
use std::path::Path;

use rusqlite::Connection;

const DATA_DECREE: &str = "Kepmendagri No 300.2.2-2138 Tahun 2025";

const PDF_URL: &str = "https://upload.wikimedia.org/wikipedia/commons/5/51/Keputusan_Menteri_Dalam_Negeri_Nomor_300.2.2-2138_Tahun_2025.pdf";

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let db_path = Path::new(&out_dir).join("locations.db");
    let data_dir = Path::new("data");
    let data_db = data_dir.join("locations.db");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=data/locations.db");
    println!("cargo:rerun-if-env-changed=WILAYAH_REFRESH_BIG");
    println!("cargo:rerun-if-env-changed=WILAYAH_VERIFY_VERBOSE");

    let village_count = if db_path.exists() {
        village_count_from_db(&db_path)
    } else if data_db.exists() {
        fs::copy(&data_db, &db_path).expect("failed to copy cached DB to OUT_DIR");
        village_count_from_db(&db_path)
    } else {
        let count = build_official(&db_path);
        // Copy official DB to data/ for future cached builds
        fs::copy(&db_path, &data_db).expect("failed to copy official DB to data/");
        count
    };

    let build_date = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    println!("cargo:rustc-env=LOCATION_DB_PATH={}", db_path.display());
    println!("cargo:rustc-env=WILAYAH_DATA_SOURCE=official");
    println!("cargo:rustc-env=WILAYAH_DATA_DECREE={}", DATA_DECREE);
    println!("cargo:rustc-env=WILAYAH_VILLAGE_COUNT={}", village_count);
    println!("cargo:rustc-env=WILAYAH_BUILD_DATE={}", build_date);
}

fn village_count_from_db(db_path: &Path) -> u32 {
    let conn = Connection::open(db_path).expect("failed to open DB for count");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM locations", [], |row| row.get(0))
        .expect("failed to query village count");
    count as u32
}

// ============================================================================
// OFFICIAL PIPELINE (Path B)
// ============================================================================

fn build_official(db_path: &Path) -> u32 {
    let pdf_path = ensure_pdf();
    let text = extract_text(&pdf_path);
    let villages = parse_villages(&text);
    let big_data = fetch_big_data();
    let merged = merge_villages(&villages, &big_data);

    // Save legacy snapshot before verification overwrites the legacy DB
    let legacy_db = Path::new("data/locations.db");
    if legacy_db.exists() && !Path::new("data/cache/legacy_snapshot.json").exists() {
        save_legacy_snapshot(legacy_db);
    }

    verify_data(&merged);

    build_db(&merged, db_path);
    merged.len() as u32
}

fn ensure_pdf() -> std::path::PathBuf {
    let cache_dir = Path::new("data/cache");
    fs::create_dir_all(cache_dir).expect("failed to create cache directory");
    let pdf_path = cache_dir.join("kemendagri.pdf");

    if !pdf_path.exists() {
        println!("cargo:warning=Downloading Kemendagri PDF (57 MB)...");
        let bytes = download_with_sha256(PDF_URL);
        fs::write(&pdf_path, bytes.data).expect("failed to write PDF");
        println!("cargo:warning=PDF SHA-256: {}", bytes.sha256);
    }

    // Verify SHA-256 if we have a known hash
    // TODO: Add known SHA-256 for verification

    pdf_path
}

fn extract_text(pdf_path: &Path) -> String {
    println!("cargo:warning=Extracting text from PDF (4,428 pages)...");
    let output = std::process::Command::new("pdftotext")
        .arg("-layout")
        .arg(pdf_path)
        .arg("-")
        .output()
        .expect("pdftotext failed");

    if !output.status.success() {
        build_panic(&format!(
            "pdftotext exited with status {:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn parse_villages(text: &str) -> Vec<VillageRecord> {
    let start = std::time::Instant::now();
    let line_count = text.lines().count();
    println!(
        "cargo:warning=Parsing village records ({} lines)...",
        line_count
    );

    let re_start = std::time::Instant::now();
    let village_code_re = regex::Regex::new(r"^(\d{2}\.\d{2}\.\d{2}\.\d{4})\s").unwrap();
    let kecamatan_code_re =
        regex::Regex::new(r"^\s*(\d{2}\.\d{2}\.\d{2})\s+\d+\s+([A-Z])").unwrap();
    let name_re = regex::Regex::new(r"\s+\d{1,3}\s+(.{1,120})").unwrap();
    let section_header_re = regex::Regex::new(r"C\.\w+\.\d+\)\s+(.+)$").unwrap();
    println!("cargo:warning=Regex compilation: {:?}", re_start.elapsed());

    let mut villages = Vec::new();
    let mut current_province = "";
    let mut current_city = "";
    let mut current_district_code = String::new();
    let mut current_district_name = String::new();

    let mut village_count = 0;
    let mut kecamatan_count = 0;
    let mut header_count = 0;

    for line in text.lines() {
        // Detect section headers: "C.a.1) Kabupaten Aceh Selatan Provinsi Aceh"
        if let Some(header) = parse_section_header(line, &section_header_re) {
            current_province = header.province;
            current_city = header.city;
            current_district_code.clear();
            current_district_name.clear();
            header_count += 1;
        }

        // Detect kecamatan markers: "11.01.01        1 Bakongan                                                                   7"
        if let Some(cap) = kecamatan_code_re.captures(line) {
            current_district_code = cap.get(1).unwrap().as_str().to_string();
            // Extract name: everything after the seq number, before trailing count
            let after_prefix = &line[cap.get(0).unwrap().start()..];
            if let Some(name_end) = after_prefix.rfind(|c: char| c.is_ascii_digit()) {
                let name_part = after_prefix[..name_end].trim();
                // Skip the leading seq number
                if let Some(name_start) = name_part.find(|c: char| c.is_ascii_alphabetic()) {
                    current_district_name = name_part[name_start..].trim().to_string();
                }
            }
            kecamatan_count += 1;
            continue;
        }

        // Detect village rows: "11.01.01.2001                                                                                   1   Keude Bakongan"
        if let Some(code) = village_code_re.captures(line).and_then(|c| c.get(1)) {
            let code_str = code.as_str().to_string();
            let district_code = code_str[..8].to_string();
            // Update district context if it changed
            if district_code != current_district_code {
                current_district_code = district_code.clone();
            }

            // Extract name: find the seq number (digits) and take text after it
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
                village_count += 1;
            }
        }
    }

    println!(
        "cargo:warning=Extracted {} village records from {} kecamatan, {} headers in {:?}",
        village_count,
        kecamatan_count,
        header_count,
        start.elapsed()
    );
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

    if let Some(cap) = name_re.captures(after_code) {
        let raw = cap.get(1)?.as_str().trim();
        if raw.is_empty() || raw.chars().next().map(|c| c.is_numeric()).unwrap_or(false) {
            None
        } else {
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
    } else {
        None
    }
}

fn parse_section_header<'a>(line: &'a str, re: &regex::Regex) -> Option<SectionHeader<'a>> {
    // Match: "C.a.1) Kabupaten Aceh Selatan Provinsi Aceh"
    if let Some(cap) = re.captures(line) {
        let text = cap.get(1)?.as_str();
        // Split into city and province - look for "Provinsi" keyword
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

const BIG_API_URL: &str =
    "https://geoservices.big.go.id/gis/rest/services/BAPANAS/Batas_Administrasi/MapServer/2/query";
const BIG_CACHE: &str = "data/cache/big_villages.json";
const BIG_BATCH_SIZE: usize = 1000;

fn fetch_big_data() -> Vec<BigRecord> {
    let cache_path = Path::new(BIG_CACHE);
    let force_refresh = std::env::var("WILAYAH_REFRESH_BIG").is_ok();

    if !force_refresh && cache_path.exists() {
        let content = fs::read_to_string(cache_path).expect("failed to read BIG cache");
        let records: Vec<serde_json::Value> =
            serde_json::from_str(&content).expect("failed to parse BIG cache");
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
        println!(
            "cargo:warning=Loaded {} BIG village records from cache",
            result.len()
        );
        return result;
    }

    println!("cargo:warning=Fetching BIG village data from ArcGIS API...");
    fs::create_dir_all("data/cache").expect("failed to create cache directory");

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
            BIG_API_URL, BIG_BATCH_SIZE, offset
        );

        if batch_num % 10 == 1 || batch_num <= 3 {
            println!(
                "cargo:warning=Fetching BIG batch {} (offset={})...",
                batch_num, offset
            );
        }

        let resp = fetch_with_retry(&url, 3);
        let json: serde_json::Value =
            serde_json::from_str(&resp).expect("failed to parse BIG API response");

        if let Some(error) = json.get("error") {
            build_panic(&format!("BIG API error: {}", error));
        }

        let features = json
            .get("features")
            .and_then(|f| f.as_array())
            .expect("missing features in BIG response");

        if features.is_empty() {
            break;
        }

        for feature in features {
            let attrs = feature.get("attributes").expect("missing attributes");
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

    println!(
        "cargo:warning=Fetched {} BIG village records in {} batches",
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
    let cache_json = serde_json::to_string(&cache_data).expect("failed to serialize BIG cache");
    fs::write(cache_path, cache_json).expect("failed to write BIG cache");
    println!("cargo:warning=Saved BIG cache to {}", BIG_CACHE);

    all_records
}

fn fetch_with_retry(url: &str, max_retries: usize) -> String {
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
                    .expect("failed to read response");
                return buf;
            }
            Err(e) => {
                last_err = format!("{}", e);
                if attempt < max_retries {
                    let wait_secs = 2_u64.pow(attempt as u32);
                    println!(
                        "cargo:warning=BIG API attempt {} failed, retrying in {}s: {}",
                        attempt + 1,
                        wait_secs,
                        last_err
                    );
                    std::thread::sleep(std::time::Duration::from_secs(wait_secs));
                }
            }
        }
    }
    build_panic(&format!(
        "BIG API failed after {} retries: {}",
        max_retries, last_err
    ))
}

fn compute_centroid(geometry: &serde_json::Value) -> (f64, f64) {
    // Handle both single polygon and multipolygon
    let mut rings: Vec<&[serde_json::Value]> = Vec::new();

    if let Some(rings_array) = geometry.get("rings").and_then(|r| r.as_array()) {
        for ring_val in rings_array {
            if let Some(ring) = ring_val.as_array() {
                rings.push(ring);
            }
        }
    } else if let Some(coord_arrays) = geometry.get("coordinates").and_then(|c| c.as_array()) {
        // Multipolygon or Polygon from GeoJSON-style
        if let Some(first) = coord_arrays.first() {
            if first.get(0).map(|r| r.is_array()).unwrap_or(false) {
                // Multipolygon: [[[[lon,lat], ...]]]
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
                // Single polygon: [[[lon,lat], ...]]
                rings.push(coord_arrays);
            }
        }
    }

    if rings.is_empty() {
        return (0.0, 0.0);
    }

    // Find the largest ring by vertex count (main boundary)
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

    // Area-weighted centroid (center of mass)
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
        // Degenerate polygon, fall back to average
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

    (cy, cx) // Return (lat, lon) not (lon, lat)
}

fn merge_villages(
    villages: &[VillageRecord],
    big_data: &[BigRecord],
) -> Vec<(String, String, String, String, String, f64, f64)> {
    // Build lookup from BIG data
    let big_lookup: std::collections::HashMap<&str, &BigRecord> =
        big_data.iter().map(|r| (r.code.as_str(), r)).collect();

    // Compute kecamatan centroids for fallback
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
            // Fallback to kecamatan centroid
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

    println!(
        "cargo:warning=Merged {} villages: {} matched BIG, {} fallback to kecamatan centroid",
        matched + fallback,
        matched,
        fallback
    );
    merged
}

// ============================================================================
// DATA VERIFICATION (Phase 8)
// ============================================================================

type VillageTuple = (String, String, String, String, String, f64, f64);

struct LegacyVillage {
    code: String,
    name: String,
    district: String,
    city: String,
    province: String,
    lat: f64,
    lon: f64,
}

struct VerificationReport {
    official_count: usize,
    legacy_count: usize,
    new_villages: Vec<VillageTuple>,
    missing_villages: Vec<LegacyVillage>,
    name_diffs: Vec<(String, String, String)>,
    coord_drifts: Vec<(String, f64)>,
    hierarchy_diffs: Vec<(String, String, String, String, String, String, String)>,
}

fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_M: f64 = 6_371_000.0;
    let to_rad = |d: f64| d * std::f64::consts::PI / 180.0;
    let dlat = to_rad(lat2 - lat1);
    let dlon = to_rad(lon2 - lon1);
    let a = (dlat / 2.0).sin().powi(2)
        + to_rad(lat1).cos() * to_rad(lat2).cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * a.sqrt().atan2((1.0 - a).sqrt())
}

fn verify_data(official: &[VillageTuple]) {
    let verbose = std::env::var("WILAYAH_VERIFY_VERBOSE").is_ok();
    let legacy_db = Path::new("data/locations.db");
    let snapshot_path = Path::new("data/cache/legacy_snapshot.json");

    if legacy_db.exists() {
        let report = verify_db_to_db(official, legacy_db, verbose);
        print_report(&report, verbose);
    } else if snapshot_path.exists() {
        let report = verify_snapshot(official, snapshot_path, verbose);
        print_report(&report, verbose);
    } else if verbose {
        println!(
            "cargo:warning=Verification: official={} (no legacy DB or snapshot for comparison)",
            official.len()
        );
    }
}

fn compare_data(
    official: &[VillageTuple],
    mut legacy_map: std::collections::HashMap<String, LegacyVillage>,
    _verbose: bool,
) -> VerificationReport {
    let original_legacy_count = legacy_map.len();
    let mut new_villages = Vec::new();
    let mut missing_villages = Vec::new();
    let mut name_diffs = Vec::new();
    let mut coord_drifts = Vec::new();
    let mut hierarchy_diffs = Vec::new();

    for v in official {
        let code = &v.0;
        if let Some(leg) = legacy_map.remove(code) {
            if v.1 != leg.name {
                name_diffs.push((code.clone(), v.1.clone(), leg.name.clone()));
            }
            let drift = haversine_distance(v.5, v.6, leg.lat, leg.lon);
            if drift > 1000.0 {
                coord_drifts.push((code.clone(), drift));
            }
            if v.2 != leg.district || v.3 != leg.city || v.4 != leg.province {
                hierarchy_diffs.push((
                    code.clone(),
                    v.2.clone(),
                    leg.district.clone(),
                    v.3.clone(),
                    leg.city.clone(),
                    v.4.clone(),
                    leg.province.clone(),
                ));
            }
        } else {
            new_villages.push(v.clone());
        }
    }

    for (_, leg) in legacy_map {
        missing_villages.push(leg);
    }

    VerificationReport {
        official_count: official.len(),
        legacy_count: original_legacy_count,
        new_villages,
        missing_villages,
        name_diffs,
        coord_drifts,
        hierarchy_diffs,
    }
}

fn print_report(report: &VerificationReport, verbose: bool) {
    println!(
        "cargo:warning=Verification: official={}, legacy={}",
        report.official_count, report.legacy_count
    );
    println!(
        "cargo:warning=  New: {} | Missing: {} | Name diffs: {} | Drift >1km: {} | Hierarchy diffs: {}",
        report.new_villages.len(),
        report.missing_villages.len(),
        report.name_diffs.len(),
        report.coord_drifts.len(),
        report.hierarchy_diffs.len()
    );

    if !verbose {
        return;
    }

    if !report.new_villages.is_empty() {
        let show = report.new_villages.iter().take(10);
        println!(
            "cargo:warning=\nNEW VILLAGES (first {} of {}):",
            show.len(),
            report.new_villages.len()
        );
        for v in show {
            println!(
                "cargo:warning=  {}  {}  ({}, {}, {})",
                v.0, v.1, v.2, v.3, v.4
            );
        }
        if report.new_villages.len() > 10 {
            println!(
                "cargo:warning=  ... and {} more",
                report.new_villages.len() - 10
            );
        }
    }

    if !report.missing_villages.is_empty() {
        let show = report.missing_villages.iter().take(10);
        println!(
            "cargo:warning=\nMISSING VILLAGES (first {} of {}):",
            show.len(),
            report.missing_villages.len()
        );
        for v in show {
            println!(
                "cargo:warning=  {}  {}  ({}, {}, {})",
                v.code, v.name, v.district, v.city, v.province
            );
        }
        if report.missing_villages.len() > 10 {
            println!(
                "cargo:warning=  ... and {} more",
                report.missing_villages.len() - 10
            );
        }
    }

    if !report.name_diffs.is_empty() {
        let show = report.name_diffs.iter().take(10);
        println!(
            "cargo:warning=\nNAME DIFFERENCES (first {} of {}):",
            show.len(),
            report.name_diffs.len()
        );
        for (code, off_name, leg_name) in show {
            println!(
                "cargo:warning=  {}  Official: \"{}\"  Legacy: \"{}\"",
                code, off_name, leg_name
            );
        }
        if report.name_diffs.len() > 10 {
            println!(
                "cargo:warning=  ... and {} more",
                report.name_diffs.len() - 10
            );
        }
    }

    if !report.coord_drifts.is_empty() {
        let show = report.coord_drifts.iter().take(10);
        println!(
            "cargo:warning=\nCOORDINATE DRIFT >1km (first {} of {}):",
            show.len(),
            report.coord_drifts.len()
        );
        for (code, drift) in show {
            println!("cargo:warning=  {}  drift={:.0}m", code, drift);
        }
        if report.coord_drifts.len() > 10 {
            println!(
                "cargo:warning=  ... and {} more",
                report.coord_drifts.len() - 10
            );
        }
    }

    if !report.hierarchy_diffs.is_empty() {
        let show = report.hierarchy_diffs.iter().take(10);
        println!(
            "cargo:warning=\nHIERARCHY DIFFERENCES (first {} of {}):",
            show.len(),
            report.hierarchy_diffs.len()
        );
        for (code, off_dist, leg_dist, off_city, leg_city, off_prov, leg_prov) in show {
            if off_dist != leg_dist {
                println!(
                    "cargo:warning=  {}  District: \"{}\" vs \"{}\"",
                    code, off_dist, leg_dist
                );
            }
            if off_city != leg_city {
                println!(
                    "cargo:warning=  {}  City: \"{}\" vs \"{}\"",
                    code, off_city, leg_city
                );
            }
            if off_prov != leg_prov {
                println!(
                    "cargo:warning=  {}  Province: \"{}\" vs \"{}\"",
                    code, off_prov, leg_prov
                );
            }
        }
        if report.hierarchy_diffs.len() > 10 {
            println!(
                "cargo:warning=  ... and {} more",
                report.hierarchy_diffs.len() - 10
            );
        }
    }
}

fn verify_db_to_db(
    official: &[VillageTuple],
    legacy_db_path: &Path,
    verbose: bool,
) -> VerificationReport {
    let conn = Connection::open(legacy_db_path).expect("failed to open legacy DB");

    let mut legacy_map: std::collections::HashMap<String, LegacyVillage> =
        std::collections::HashMap::new();
    let mut stmt = conn
        .prepare("SELECT kode, nama, kecamatan, kota, provinsi, lat, lon FROM locations")
        .expect("failed to prepare legacy query");

    let mut rows = stmt.query([]).expect("failed to query legacy DB");
    while let Some(row) = rows.next().expect("failed to fetch legacy row") {
        let code: String = row.get(0).expect("kode");
        legacy_map.insert(
            code.clone(),
            LegacyVillage {
                code,
                name: row.get(1).expect("nama"),
                district: row.get(2).expect("kecamatan"),
                city: row.get(3).expect("kota"),
                province: row.get(4).expect("provinsi"),
                lat: row.get(5).expect("lat"),
                lon: row.get(6).expect("lon"),
            },
        );
    }

    compare_data(official, legacy_map, verbose)
}

fn verify_snapshot(
    official: &[VillageTuple],
    snapshot_path: &Path,
    verbose: bool,
) -> VerificationReport {
    let content = fs::read_to_string(snapshot_path).expect("failed to read legacy snapshot");
    let records: Vec<serde_json::Value> =
        serde_json::from_str(&content).expect("failed to parse legacy snapshot");

    let mut legacy_map: std::collections::HashMap<String, LegacyVillage> =
        std::collections::HashMap::with_capacity(records.len());
    for r in records {
        if let (Some(code), Some(lat), Some(lon)) = (
            r.get("code").and_then(|v| v.as_str()),
            r.get("lat").and_then(|v| v.as_f64()),
            r.get("lon").and_then(|v| v.as_f64()),
        ) {
            legacy_map.insert(
                code.to_string(),
                LegacyVillage {
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
                },
            );
        }
    }

    compare_data(official, legacy_map, verbose)
}

fn save_legacy_snapshot(legacy_db_path: &Path) {
    let snapshot_path = Path::new("data/cache/legacy_snapshot.json");
    fs::create_dir_all("data/cache").expect("failed to create cache directory");

    let conn = Connection::open(legacy_db_path).expect("failed to open legacy DB");
    let mut stmt = conn
        .prepare("SELECT kode, nama, kecamatan, kota, provinsi, lat, lon FROM locations")
        .expect("failed to prepare legacy query");

    let mut rows = stmt.query([]).expect("failed to query legacy DB");
    let mut snapshot: Vec<serde_json::Value> = Vec::new();
    while let Some(row) = rows.next().expect("failed to fetch row") {
        snapshot.push(serde_json::json!({
            "code": row.get::<_, String>(0).expect("kode"),
            "name": row.get::<_, String>(1).expect("nama"),
            "district": row.get::<_, String>(2).expect("kecamatan"),
            "city": row.get::<_, String>(3).expect("kota"),
            "province": row.get::<_, String>(4).expect("provinsi"),
            "lat": row.get::<_, f64>(5).expect("lat"),
            "lon": row.get::<_, f64>(6).expect("lon"),
        }));
    }

    let json = serde_json::to_string(&snapshot).expect("failed to serialize legacy snapshot");
    fs::write(snapshot_path, json).expect("failed to write legacy snapshot");
    println!(
        "cargo:warning=Saved legacy snapshot with {} villages to {}",
        snapshot.len(),
        snapshot_path.display()
    );
}

struct DownloadResult {
    data: Vec<u8>,
    sha256: String,
}

fn download_with_sha256(url: &str) -> DownloadResult {
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(300))
        .call()
        .unwrap_or_else(|e| build_panic(&format!("Failed to download {}: {}", url, e)));

    let mut reader = resp.into_reader();
    let mut data = Vec::new();
    reader
        .read_to_end(&mut data)
        .expect("failed to read response");

    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(&data);
    let sha256 = format!("{:x}", hasher.finalize());

    DownloadResult { data, sha256 }
}

fn build_db(villages: &[VillageTuple], db_path: &Path) {
    if db_path.exists() {
        fs::remove_file(db_path).unwrap();
    }

    let mut conn = Connection::open(db_path).expect("failed to create DB");
    conn.execute_batch(
        "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA page_size = 4096;",
    )
    .expect("PRAGMA failed");

    conn.execute(
        "CREATE TABLE locations (
            id INTEGER PRIMARY KEY, kode TEXT NOT NULL UNIQUE, nama TEXT NOT NULL,
            kecamatan TEXT NOT NULL, kota TEXT NOT NULL, provinsi TEXT NOT NULL,
            lat REAL NOT NULL, lon REAL NOT NULL
        )",
        [],
    )
    .expect("failed to create locations");

    conn.execute(
        "CREATE VIRTUAL TABLE geo_rtree USING rtree(id, min_lon, max_lon, min_lat, max_lat)",
        [],
    )
    .expect("failed to create RTree");

    conn.execute(
        "CREATE VIRTUAL TABLE locations_fts USING fts5(
            nama, kecamatan, kota, provinsi, content='locations', content_rowid='id'
        )",
        [],
    )
    .expect("failed to create FTS5");

    conn.execute("CREATE INDEX idx_locations_nama ON locations(nama)", [])
        .expect("failed to create nama index");
    conn.execute(
        "CREATE UNIQUE INDEX idx_locations_kode ON locations(kode)",
        [],
    )
    .expect("failed to create kode index");

    let tx = conn.transaction().expect("failed to begin transaction");
    {
        let mut ins_loc = tx
            .prepare("INSERT INTO locations (id, kode, nama, kecamatan, kota, provinsi, lat, lon) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")
            .expect("prepare insert locations");
        let mut ins_rtree = tx
            .prepare("INSERT INTO geo_rtree (id, min_lon, max_lon, min_lat, max_lat) VALUES (?1, ?2, ?3, ?4, ?5)")
            .expect("prepare insert rtree");

        for (i, (kode, nama, kecamatan, kota, provinsi, lat, lon)) in villages.iter().enumerate() {
            let rowid = (i + 1) as i64;
            ins_loc
                .execute(rusqlite::params![
                    rowid, kode, nama, kecamatan, kota, provinsi, lat, lon
                ])
                .expect("insert location");
            ins_rtree
                .execute(rusqlite::params![rowid, lon, lon, lat, lat])
                .expect("insert rtree");
        }
    }
    tx.commit().expect("failed to commit transaction");

    conn.execute(
        "INSERT INTO locations_fts(locations_fts) VALUES('rebuild')",
        [],
    )
    .expect("failed to rebuild FTS5");

    conn.execute_batch("PRAGMA analysis_limit = 400; PRAGMA optimize; VACUUM;")
        .expect("optimize failed");

    let size = fs::metadata(db_path).unwrap().len();
    println!(
        "cargo:warning=Database written: {:.1} MB",
        size as f64 / (1024.0 * 1024.0)
    );
}

fn build_panic(msg: &str) -> ! {
    panic!("{}", msg);
}
