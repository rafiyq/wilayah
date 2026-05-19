use std::fs;
use std::io::Read;
use std::path::Path;

#[path = "src/pipeline.rs"]
mod pipeline;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let db_path = Path::new(&out_dir).join("locations.db");
    let data_dir = Path::new("data");
    let data_db = data_dir.join("locations.db");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=data/locations.db");
    println!("cargo:rerun-if-env-changed=WILAYAH_REFRESH_BIG");
    println!("cargo:rerun-if-env-changed=WILAYAH_VERIFY_VERBOSE");

    // Download mode (default): copy local data/locations.db or download from GitHub Release
    // Pipeline mode: set WILAYAH_BUILD_PIPELINE=1 to run full pipeline
    let pipeline_enabled = std::env::var("WILAYAH_BUILD_PIPELINE").is_ok();

    let village_count = if pipeline_enabled {
        eprintln!("Pipeline mode: running official build...");
        let output = pipeline::Pipeline::new()
            .output(&data_db)
            .run()
            .expect("Pipeline failed");
        // Copy result to OUT_DIR for cargo
        fs::copy(&data_db, &db_path).expect("failed to copy DB to OUT_DIR");
        output.village_count as u32
    } else if db_path.exists() {
        village_count_from_db(&db_path)
    } else if data_db.exists() {
        fs::copy(&data_db, &db_path).expect("failed to copy cached DB to OUT_DIR");
        village_count_from_db(&db_path)
    } else {
        // Try to download pre-built database from GitHub Releases
        eprintln!("Downloading pre-built database from GitHub Releases...");
        match download_latest_db(&db_path) {
            Ok(()) => {
                // Also cache for future builds
                let _ = fs::create_dir_all("data");
                let _ = fs::copy(&db_path, &data_db);
                village_count_from_db(&db_path)
            }
            Err(e) => {
                panic!(
                    "Failed to download pre-built database: {}\n\
                     To build from source, run:\n\
                     cargo run --example build_db --features build-db\n\
                     Then re-run cargo build.",
                    e
                );
            }
        }
    };

    let build_date = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // If pipeline ran, also set environment variables with metadata
    if pipeline_enabled {
        println!("cargo:rustc-env=WILAYAH_DATA_SOURCE=official");
        println!(
            "cargo:rustc-env=WILAYAH_DATA_DECREE={}",
            pipeline::DATA_DECREE
        );
    } else {
        // In download mode, we don't know these with certainty, so set generic values
        println!("cargo:rustc-env=WILAYAH_DATA_SOURCE=release");
        println!("cargo:rustc-env=WILAYAH_DATA_DECREE=unknown");
    }

    println!("cargo:rustc-env=LOCATION_DB_PATH={}", db_path.display());
    println!("cargo:rustc-env=WILAYAH_VILLAGE_COUNT={}", village_count);
    println!("cargo:rustc-env=WILAYAH_BUILD_DATE={}", build_date);
}

fn village_count_from_db(db_path: &Path) -> u32 {
    use rusqlite::Connection;
    let conn = Connection::open(db_path).expect("failed to open DB for count");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM locations", [], |row| row.get(0))
        .expect("failed to query village count");
    count as u32
}

fn download_latest_db(dest: &Path) -> Result<(), String> {
    let url = "https://github.com/rafiyq/wilayah/releases/latest/download/locations.db";
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(300))
        .call()
        .map_err(|e| format!("{}", e))?;
    let status = resp.status();
    if !(200..=299).contains(&status) {
        return Err(format!("HTTP {} {}", status, resp.status_text()));
    }
    let mut data = Vec::new();
    resp.into_reader()
        .read_to_end(&mut data)
        .map_err(|e| format!("{}", e))?;
    fs::write(dest, data).map_err(|e| format!("{}", e))?;
    Ok(())
}
