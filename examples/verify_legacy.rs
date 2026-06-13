use std::collections::HashMap;
use std::fs;
use std::path::Path;
use wilayah::haversine_km;

fn main() {
    let snapshot_path = Path::new("data/cache/legacy_snapshot.json");

    let db = match wilayah::Database::open() {
        Ok(db) => db,
        Err(e) => {
            eprintln!("error: failed to open embedded DB: {}", e);
            std::process::exit(1);
        }
    };

    let snapshot: Vec<serde_json::Value> = if snapshot_path.exists() {
        let content = fs::read_to_string(snapshot_path).expect("failed to read legacy snapshot");
        serde_json::from_str(&content).expect("failed to parse legacy snapshot")
    } else {
        eprintln!(
            "error: legacy snapshot not found at {}",
            snapshot_path.display()
        );
        eprintln!("Run the pipeline once to generate it.");
        std::process::exit(1);
    };

    let conn = db.conn_guard();
    let mut official_map: HashMap<String, VillageEntry> = HashMap::new();
    let mut stmt = conn
        .prepare("SELECT kode, nama, kecamatan, kota, provinsi, lat, lon FROM locations")
        .expect("failed to prepare official query");
    let mut rows = stmt.query([]).expect("failed to query official DB");
    while let Some(row) = rows.next().expect("failed to fetch official row") {
        let code: String = row.get(0).expect("kode");
        official_map.insert(
            code.clone(),
            VillageEntry {
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

    // Load snapshot into map
    let mut legacy_map: HashMap<String, VillageEntry> = HashMap::new();
    for r in snapshot {
        if let (Some(code), Some(lat), Some(lon)) = (
            r.get("code").and_then(|v| v.as_str()),
            r.get("lat").and_then(|v| v.as_f64()),
            r.get("lon").and_then(|v| v.as_f64()),
        ) {
            legacy_map.insert(
                code.to_string(),
                VillageEntry {
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

    // Compare
    let report = compare_data(&official_map, legacy_map);

    // Print report
    println!(
        "Verification: official={}, legacy={}",
        report.official_count, report.legacy_count
    );
    println!(
        "  New: {} | Missing: {} | Name diffs: {} | Drift >1km: {} | Hierarchy diffs: {}",
        report.new_villages.len(),
        report.missing_villages.len(),
        report.name_diffs.len(),
        report.coord_drifts.len(),
        report.hierarchy_diffs.len()
    );

    // Verbose details
    if !report.new_villages.is_empty() {
        println!(
            "\nNEW VILLAGES (first 10 of {}):",
            report.new_villages.len()
        );
        for v in report.new_villages.iter().take(10) {
            println!(
                "  {}  {}  ({}, {}, {})",
                v.code, v.name, v.district, v.city, v.province
            );
        }
        if report.new_villages.len() > 10 {
            println!("  ... and {} more", report.new_villages.len() - 10);
        }
    }

    if !report.missing_villages.is_empty() {
        println!(
            "\nMISSING VILLAGES (first 10 of {}):",
            report.missing_villages.len()
        );
        for v in report.missing_villages.iter().take(10) {
            println!(
                "  {}  {}  ({}, {}, {})",
                v.code, v.name, v.district, v.city, v.province
            );
        }
        if report.missing_villages.len() > 10 {
            println!("  ... and {} more", report.missing_villages.len() - 10);
        }
    }

    if !report.name_diffs.is_empty() {
        println!(
            "\nNAME DIFFERENCES (first 10 of {}):",
            report.name_diffs.len()
        );
        for (code, off_name, leg_name) in report.name_diffs.iter().take(10) {
            println!(
                "  {}  Official: \"{}\"  Legacy: \"{}\"",
                code, off_name, leg_name
            );
        }
        if report.name_diffs.len() > 10 {
            println!("  ... and {} more", report.name_diffs.len() - 10);
        }
    }

    if !report.coord_drifts.is_empty() {
        println!(
            "\nCOORDINATE DRIFT >1km (first 10 of {}):",
            report.coord_drifts.len()
        );
        for (code, drift) in report.coord_drifts.iter().take(10) {
            println!("  {}  drift={:.0}m", code, drift);
        }
        if report.coord_drifts.len() > 10 {
            println!("  ... and {} more", report.coord_drifts.len() - 10);
        }
    }

    if !report.hierarchy_diffs.is_empty() {
        println!(
            "\nHIERARCHY DIFFERENCES (first 10 of {}):",
            report.hierarchy_diffs.len()
        );
        for (code, off_dist, leg_dist, off_city, leg_city, off_prov, leg_prov) in
            report.hierarchy_diffs.iter().take(10)
        {
            if off_dist != leg_dist {
                println!("  {}  District: \"{}\" vs \"{}\"", code, off_dist, leg_dist);
            }
            if off_city != leg_city {
                println!("  {}  City: \"{}\" vs \"{}\"", code, off_city, leg_city);
            }
            if off_prov != leg_prov {
                println!("  {}  Province: \"{}\" vs \"{}\"", code, off_prov, leg_prov);
            }
        }
        if report.hierarchy_diffs.len() > 10 {
            println!("  ... and {} more", report.hierarchy_diffs.len() - 10);
        }
    }
}

#[derive(Clone)]
struct VillageEntry {
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
    new_villages: Vec<VillageEntry>,
    missing_villages: Vec<VillageEntry>,
    name_diffs: Vec<(String, String, String)>,
    coord_drifts: Vec<(String, f64)>,
    hierarchy_diffs: Vec<(String, String, String, String, String, String, String)>,
}

fn compare_data(
    official_map: &HashMap<String, VillageEntry>,
    mut legacy_map: HashMap<String, VillageEntry>,
) -> VerificationReport {
    let original_legacy_count = legacy_map.len();
    let mut new_villages = Vec::new();
    let mut missing_villages = Vec::new();
    let mut name_diffs = Vec::new();
    let mut coord_drifts = Vec::new();
    let mut hierarchy_diffs = Vec::new();

    for (code, official) in official_map {
        if let Some(legacy) = legacy_map.remove(code) {
            if official.name != legacy.name {
                name_diffs.push((code.clone(), official.name.clone(), legacy.name.clone()));
            }
            let drift = haversine_km(official.lat, official.lon, legacy.lat, legacy.lon) * 1000.0;
            if drift > 1000.0 {
                coord_drifts.push((code.clone(), drift));
            }
            if official.district != legacy.district
                || official.city != legacy.city
                || official.province != legacy.province
            {
                hierarchy_diffs.push((
                    code.clone(),
                    official.district.clone(),
                    legacy.district.clone(),
                    official.city.clone(),
                    legacy.city.clone(),
                    official.province.clone(),
                    legacy.province.clone(),
                ));
            }
        } else {
            new_villages.push(official.clone());
        }
    }

    for (_, legacy) in legacy_map {
        missing_villages.push(legacy);
    }

    VerificationReport {
        official_count: official_map.len(),
        legacy_count: original_legacy_count,
        new_villages,
        missing_villages,
        name_diffs,
        coord_drifts,
        hierarchy_diffs,
    }
}
