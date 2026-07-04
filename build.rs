use std::fs;
use std::io::Read;
use std::path::Path;

const DB_DOWNLOAD_URL: &str =
    "https://github.com/rafiyq/wilayah/releases/latest/download/locations.db";

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let db_path = Path::new(&out_dir).join("locations.db");
    let data_dir = Path::new("data");
    let data_db = data_dir.join("locations.db");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=data/locations.db");

    if db_path.exists() {
        if let Ok(data_mtime) = fs::metadata(&data_db).and_then(|m| m.modified()) {
            if let Ok(out_mtime) = fs::metadata(&db_path).and_then(|m| m.modified()) {
                if data_mtime > out_mtime {
                    fs::copy(&data_db, &db_path).expect("failed to update OUT_DIR DB from data/");
                }
            }
        }
    } else if data_db.exists() {
        fs::copy(&data_db, &db_path).expect("failed to copy cached DB to OUT_DIR");
    } else {
        eprintln!("Downloading pre-built database from GitHub Releases...");
        match download_latest_db(&db_path) {
            Ok(()) => {}
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
    }

    println!("cargo:rustc-env=LOCATION_DB_PATH={}", db_path.display());
}

fn download_latest_db(dest: &Path) -> Result<(), String> {
    let resp = ureq::get(DB_DOWNLOAD_URL)
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
