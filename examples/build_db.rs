use rusqlite::Connection;
use serde_json::{json, Value};
use wilayah::builder::Pipeline;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut pipeline = Pipeline::new();
    let mut save_legacy_snapshot = false;

    // Parse simple flags (not using clap for minimalism)
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    pipeline = pipeline.output(std::path::Path::new(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("error: --output requires a path");
                    std::process::exit(1);
                }
            }
            "--cache-dir" => {
                if i + 1 < args.len() {
                    pipeline = pipeline.cache_dir(std::path::Path::new(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("error: --cache-dir requires a path");
                    std::process::exit(1);
                }
            }
            "--pdf-url" => {
                if i + 1 < args.len() {
                    pipeline = pipeline.pdf_url(&args[i + 1]);
                    i += 2;
                } else {
                    eprintln!("error: --pdf-url requires a URL");
                    std::process::exit(1);
                }
            }
            "--big-api-url" => {
                if i + 1 < args.len() {
                    pipeline = pipeline.big_api_url(&args[i + 1]);
                    i += 2;
                } else {
                    eprintln!("error: --big-api-url requires a URL");
                    std::process::exit(1);
                }
            }
            "--force-refresh-big" => {
                pipeline = pipeline.force_refresh_big(true);
                i += 1;
            }
            "--save-legacy-snapshot" => {
                save_legacy_snapshot = true;
                i += 1;
            }
            _ => {
                eprintln!("warning: unknown argument: {}", args[i]);
                i += 1;
            }
        }
    }

    match pipeline.run() {
        Ok(output) => {
            println!("Pipeline completed successfully.");
            println!("Database: {}", output.db_path.display());
            println!("Villages: {}", output.village_count);
            println!("SHA-256: {}", output.sha256);

            if save_legacy_snapshot {
                if let Err(e) = save_legacy_snapshot_to(&output.db_path) {
                    eprintln!("Failed to save legacy snapshot: {}", e);
                    std::process::exit(1);
                }
            }

            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Pipeline failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn save_legacy_snapshot_to(db_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::open(db_path)?;

    let mut stmt = conn.prepare(
        "SELECT kode, nama, kecamatan, kota, provinsi, lat, lon FROM locations ORDER BY kode",
    )?;

    let mut snapshot = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let village: Value = json!({
            "code": row.get::<_, String>(0)?,
            "name": row.get::<_, String>(1)?,
            "district": row.get::<_, String>(2)?,
            "city": row.get::<_, String>(3)?,
            "province": row.get::<_, String>(4)?,
            "lat": row.get::<_, f64>(5)?,
            "lon": row.get::<_, f64>(6)?,
        });
        snapshot.push(village);
    }

    let snapshot_path = std::path::Path::new("data/cache/legacy_snapshot.json");
    std::fs::create_dir_all(snapshot_path.parent().unwrap())?;
    let file = std::fs::File::create(snapshot_path)?;
    serde_json::to_writer_pretty(file, &snapshot)?;

    eprintln!("Saved legacy snapshot to {}", snapshot_path.display());
    Ok(())
}
