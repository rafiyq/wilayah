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

    if pipeline_enabled {
        eprintln!("Pipeline mode: running official build...");
        let output = pipeline::Pipeline::new()
            .output(&db_path)
            .run()
            .expect("Pipeline failed");
        eprintln!("Pipeline produced {} villages", output.village_count);
    } else if db_path.exists() {
        // Already have DB in OUT_DIR from previous build
    } else if data_db.exists() {
        fs::copy(&data_db, &db_path).expect("failed to copy cached DB to OUT_DIR");
    } else {
        // Try to download pre-built database from GitHub Releases
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
