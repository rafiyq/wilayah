use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use regex::Regex;
use rusqlite::Connection;

const WILAYAH_URL: &str =
    "https://raw.githubusercontent.com/cahyadsn/wilayah/master/db/wilayah.sql";
const GITHUB_TREE_URL: &str =
    "https://api.github.com/repos/cahyadsn/wilayah_boundaries/git/trees/main?recursive=1";

const DATA_SOURCE: &str = "cahyadsn/wilayah_boundaries";
const DATA_DECREE: &str = "Kepmendagri No 300.2.2-2430 Tahun 2025";

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let db_path = Path::new(&out_dir).join("locations.db");

    let data_dir = Path::new("data");
    let data_db = data_dir.join("locations.db");

    println!("cargo:rerun-if-changed=data/wilayah.sql");
    println!("cargo:rerun-if-changed=build.rs");

    let village_count: u32;

    if db_path.exists() {
        village_count = village_count_from_db(&db_path);
    } else if data_db.exists() {
        std::fs::copy(&data_db, &db_path).expect("failed to copy cached DB to OUT_DIR");
        village_count = village_count_from_db(&db_path);
    } else {
        // Check WILAYAH_DATA_DIR for a pre-built DB
        if let Ok(dir) = std::env::var("WILAYAH_DATA_DIR") {
            let cached_db = Path::new(&dir).join("locations.db");
            if cached_db.exists() {
                std::fs::copy(&cached_db, &db_path)
                    .expect("failed to copy WILAYAH_DATA_DIR DB to OUT_DIR");
                village_count = village_count_from_db(&db_path);
            } else {
                println!("cargo:warning=Building locations.db from raw data (first build, this takes a minute)...");
                let raw_dir = Path::new(&dir).to_path_buf();
                village_count = download_and_build(&raw_dir, &data_db);
                std::fs::copy(&data_db, &db_path).expect("failed to copy DB to OUT_DIR");
            }
        } else {
            println!("cargo:warning=Building locations.db from raw data (first build, this takes a minute)...");
            let raw_dir = Path::new("data").join("raw");
            village_count = download_and_build(&raw_dir, &data_db);
            std::fs::copy(&data_db, &db_path).expect("failed to copy DB to OUT_DIR");
        }
    }

    let build_date = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    println!("cargo:rustc-env=LOCATION_DB_PATH={}", db_path.display());
    println!("cargo:rustc-env=WILAYAH_DATA_SOURCE={}", DATA_SOURCE);
    println!("cargo:rustc-env=WILAYAH_DATA_DECREE={}", DATA_DECREE);
    println!("cargo:rustc-env=WILAYAH_VILLAGE_COUNT={}", village_count);
    println!("cargo:rustc-env=WILAYAH_BUILD_DATE={}", build_date);
    println!("cargo:rerun-if-env-changed=WILAYAH_DATA_DIR");
}

fn village_count_from_db(db_path: &Path) -> u32 {
    let conn = Connection::open(db_path).expect("failed to open DB for count");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM locations", [], |row| row.get(0))
        .expect("failed to query village count");
    count as u32
}

fn download_and_build(raw_dir: &Path, db_path: &Path) -> u32 {
    let wilayah_path = if let Ok(dir) = std::env::var("WILAYAH_DATA_DIR") {
        Path::new(&dir).join("wilayah.sql")
    } else {
        raw_dir
            .parent()
            .unwrap_or_else(|| raw_dir)
            .join("wilayah.sql")
    };
    if !wilayah_path.exists() {
        println!("cargo:warning=Downloading wilayah.sql...");
        let bytes = download(WILAYAH_URL, "wilayah.sql");
        fs::write(&wilayah_path, bytes).expect("failed to write wilayah.sql");
    }

    if !raw_dir.exists() {
        fs::create_dir_all(raw_dir).expect("failed to create raw data directory");
    }

    let kel_urls = fetch_kel_urls();
    let mut downloaded = 0;
    for (filename, url) in &kel_urls {
        let local = raw_dir.join(filename);
        if !local.exists() {
            if downloaded % 100 == 0 {
                println!(
                    "cargo:warning=Downloading {}/{} kel files...",
                    downloaded,
                    kel_urls.len()
                );
            }
            let bytes = download(url, filename);
            fs::write(&local, bytes).expect(&format!("failed to write {filename}"));
            downloaded += 1;
            if downloaded % 50 == 0 {
                sleep(Duration::from_millis(100));
            }
        }
    }
    println!("cargo:warning=All {} kel files available.", kel_urls.len());

    let wilayah = parse_wilayah(&wilayah_path);
    println!("cargo:warning=Loaded {} hierarchy entries.", wilayah.len());

    let villages = parse_kel_files(raw_dir, &wilayah);
    println!(
        "cargo:warning=Found {} villages with valid hierarchy.",
        villages.len()
    );

    let count = villages.len() as u32;
    build_db(&villages, db_path);
    count
}

fn download(url: &str, name: &str) -> Vec<u8> {
    match ureq::get(url)
        .timeout(std::time::Duration::from_secs(120))
        .call()
    {
        Ok(resp) => {
            let mut reader = resp.into_reader();
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut reader, &mut buf).unwrap_or_else(|e| {
                build_panic(&format!("failed to read response body for {}: {}", name, e))
            });
            buf
        }
        Err(e) => {
            let msg = match e {
                ureq::Error::Status(403, _) => format!(
                    "GitHub rate limited while fetching {}. \
                     Try again in a few minutes, or set WILAYAH_DATA_DIR \
                     to a directory containing pre-downloaded kel/*.sql files.",
                    name
                ),
                ureq::Error::Status(code, _) => {
                    format!("HTTP {} while fetching {}. URL: {}", code, name, url)
                }
                ureq::Error::Transport(t) => format!(
                    "Network error fetching {}: {}. \
                     Check your internet connection, or set WILAYAH_DATA_DIR \
                     to a directory with pre-downourced data files.",
                    name, t
                ),
            };
            build_panic(&msg);
        }
    }
}

fn fetch_kel_urls() -> Vec<(String, String)> {
    let resp = match ureq::get(GITHUB_TREE_URL)
        .timeout(std::time::Duration::from_secs(30))
        .call()
    {
        Ok(r) => r,
        Err(e) => {
            let msg = match e {
                ureq::Error::Status(403, _) => {
                    "GitHub rate limited while fetching repository file list. \
                     Try again in a few minutes, or set WILAYAH_DATA_DIR \
                     to a directory containing pre-downloaded kel/*.sql files."
                        .to_string()
                }
                ureq::Error::Transport(t) => format!(
                    "Network error fetching repository file list: {}. \
                     Check your internet connection, or set WILAYAH_DATA_DIR.",
                    t
                ),
                other => format!("Error fetching repository file list: {}", other),
            };
            build_panic(&msg);
        }
    };
    let json: serde_json::Value = serde_json::from_reader(resp.into_reader())
        .unwrap_or_else(|e| build_panic(&format!("Failed to parse repository tree JSON: {}", e)));

    let mut urls = Vec::new();
    if let Some(tree) = json.get("tree").and_then(|t| t.as_array()) {
        for item in tree {
            if let Some(path) = item.get("path").and_then(|p| p.as_str()) {
                if path.starts_with("db/kel/") && path.ends_with(".sql") {
                    let filename = Path::new(path)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();
                    let raw_url = format!(
                        "https://raw.githubusercontent.com/cahyadsn/wilayah_boundaries/main/{}",
                        path
                    );
                    urls.push((filename, raw_url));
                }
            }
        }
    }
    urls.sort();
    urls
}

fn parse_wilayah(path: &Path) -> std::collections::HashMap<String, String> {
    let content = fs::read_to_string(path).expect("failed to read wilayah.sql");
    let re = Regex::new(r"\('([^']+)','([^']+)'\)").unwrap();
    let mut lookup = std::collections::HashMap::new();
    for cap in re.captures_iter(&content) {
        let kode = cap[1].to_string();
        let nama = cap[2].to_string();
        lookup.insert(kode, nama);
    }
    lookup
}

fn get_parent_names(
    kode: &str,
    wilayah: &std::collections::HashMap<String, String>,
) -> Option<(String, String, String)> {
    let parts: Vec<&str> = kode.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let province = wilayah.get(parts[0])?.clone();
    let city = wilayah.get(&format!("{}.{}", parts[0], parts[1]))?.clone();
    let district = wilayah
        .get(&format!("{}.{}.{}", parts[0], parts[1], parts[2]))?
        .clone();
    Some((province, city, district))
}

fn parse_kel_files(
    raw_dir: &Path,
    wilayah: &std::collections::HashMap<String, String>,
) -> Vec<(String, String, String, String, String, f64, f64)> {
    let pattern = Regex::new(r"\('([^']+)','([^']+)',(-?[\d.]+),(-?[\d.]+),'[^']*'\)").unwrap();
    let mut villages = Vec::new();

    let entries: Vec<_> = fs::read_dir(raw_dir)
        .expect("failed to read raw dir")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "sql")
                .unwrap_or(false)
        })
        .collect();

    println!("cargo:warning=Parsing {} kel files...", entries.len());

    for entry in &entries {
        let content = fs::read_to_string(entry.path()).expect("failed to read kel file");
        for cap in pattern.captures_iter(&content) {
            let kode = cap[1].to_string();
            let nama = cap[2].to_string();
            let lat = cap[3].parse::<f64>().expect("invalid lat");
            let lon = cap[4].parse::<f64>().expect("invalid lon");
            if let Some((province, city, district)) = get_parent_names(&kode, wilayah) {
                villages.push((kode, nama, district, city, province, lat, lon));
            }
        }
    }

    villages
}

fn build_db(villages: &[(String, String, String, String, String, f64, f64)], db_path: &Path) {
    if db_path.exists() {
        fs::remove_file(db_path).unwrap();
    }

    let mut conn = Connection::open(db_path).expect("failed to create DB");
    conn.execute_batch(
        "
        PRAGMA journal_mode = OFF;
        PRAGMA synchronous = OFF;
        PRAGMA page_size = 4096;
        ",
    )
    .expect("PRAGMA failed");

    conn.execute(
        "CREATE TABLE locations (
            id INTEGER PRIMARY KEY,
            kode TEXT NOT NULL UNIQUE,
            nama TEXT NOT NULL,
            kecamatan TEXT NOT NULL,
            kota TEXT NOT NULL,
            provinsi TEXT NOT NULL,
            lat REAL NOT NULL,
            lon REAL NOT NULL
        )",
        [],
    )
    .expect("failed to create locations");

    conn.execute(
        "CREATE VIRTUAL TABLE geo_rtree USING rtree(
            id, min_lon, max_lon, min_lat, max_lat
        )",
        [],
    )
    .expect("failed to create RTree");

    conn.execute(
        "CREATE VIRTUAL TABLE locations_fts USING fts5(
            nama, kecamatan, kota, provinsi,
            content='locations', content_rowid='id'
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
            .prepare(
                "INSERT INTO locations (id, kode, nama, kecamatan, kota, provinsi, lat, lon) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .expect("prepare insert locations");
        let mut ins_rtree = tx
            .prepare(
                "INSERT INTO geo_rtree (id, min_lon, max_lon, min_lat, max_lat) VALUES (?1, ?2, ?3, ?4, ?5)",
            )
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
    println!("cargo:warning=BUILD FAILED: {}", msg);
    println!("cargo:warning=For offline builds, set WILAYAH_DATA_DIR to a directory");
    println!("cargo:warning=containing wilayah.sql and kel/*.sql files.");
    panic!("{}", msg);
}
