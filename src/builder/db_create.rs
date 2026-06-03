//! Database construction (main DB + polygon DB) and village merging.

use super::big_api::BigRecord;
use super::geometry;
use super::parse::VillageRecord;
use super::PipelineError;
use super::PipelineResultExt;
use super::RingClassification;
use rusqlite::Connection;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// A merged village record combining PDF and BIG data.
pub(crate) struct MergedVillage {
    pub(crate) kode: String,
    pub(crate) nama: String,
    pub(crate) kecamatan: String,
    pub(crate) kota: String,
    pub(crate) provinsi: String,
    pub(crate) lat: f64,
    pub(crate) lon: f64,
}

/// Merge PDF village records with BIG polygon data.
///
/// Villages matched by code get BIG coordinates. Unmatched villages
/// fall back to the kecamatan centroid (average of all BIG villages
/// in the same kecamatan).
pub(crate) fn merge_villages(
    villages: &[VillageRecord],
    big_data: &[BigRecord],
) -> Vec<MergedVillage> {
    let big_lookup: HashMap<&str, &BigRecord> =
        big_data.iter().map(|r| (r.code.as_str(), r)).collect();

    let mut kecamatan_coords: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
    for r in big_data {
        let kec_key = format!("{}|{}|{}", r.province, r.city, r.district);
        kecamatan_coords
            .entry(kec_key)
            .or_default()
            .push((r.lat, r.lon));
    }
    let kecamatan_centroids: HashMap<String, (f64, f64)> = kecamatan_coords
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
            merged.push(MergedVillage {
                kode: v.code.clone(),
                nama: v.name.clone(),
                kecamatan: v.district.clone(),
                kota: v.city.clone(),
                provinsi: v.province.clone(),
                lat: big.lat,
                lon: big.lon,
            });
            matched += 1;
        } else {
            let kec_key = format!("{}|{}|{}", v.province, v.city, v.district);
            let (lat, lon) = kecamatan_centroids
                .get(&kec_key)
                .copied()
                .unwrap_or((0.0, 0.0));
            merged.push(MergedVillage {
                kode: v.code.clone(),
                nama: v.name.clone(),
                kecamatan: v.district.clone(),
                kota: v.city.clone(),
                provinsi: v.province.clone(),
                lat,
                lon,
            });
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

/// Build the main SQLite database with RTree and FTS5 indexes.
pub(crate) fn build_db(
    villages: &[MergedVillage],
    db_path: &Path,
    decree: &str,
    source: &str,
    build_date: u64,
) -> Result<(), PipelineError> {
    if db_path.exists() {
        fs::remove_file(db_path).ctx("failed to remove existing DB")?;
    }

    let mut conn = Connection::open(db_path).ctx("failed to create DB")?;
    create_db_schema(&conn)?;
    insert_db_meta(&conn, decree, source, build_date, villages.len())?;
    insert_villages(&mut conn, villages)?;

    conn.execute(
        "INSERT INTO locations_fts(locations_fts) VALUES('rebuild')",
        [],
    )
    .ctx("failed to rebuild FTS5")?;

    optimize_db(&conn)?;

    let size = fs::metadata(db_path).ctx("failed to get DB metadata")?;
    eprintln!(
        "Database written: {:.1} MB",
        size.len() as f64 / (1024.0 * 1024.0)
    );

    Ok(())
}

fn create_db_schema(conn: &Connection) -> Result<(), PipelineError> {
    conn.execute_batch(
        "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA page_size = 4096;",
    )
    .ctx("PRAGMA failed")?;

    conn.execute(
        "CREATE TABLE locations (
            id INTEGER PRIMARY KEY, kode TEXT NOT NULL UNIQUE, nama TEXT NOT NULL,
            kecamatan TEXT NOT NULL, kota TEXT NOT NULL, provinsi TEXT NOT NULL,
            lat REAL NOT NULL, lon REAL NOT NULL
        )",
        [],
    )
    .ctx("failed to create locations table")?;

    conn.execute(
        "CREATE VIRTUAL TABLE geo_rtree USING rtree(id, min_lon, max_lon, min_lat, max_lat)",
        [],
    )
    .ctx("failed to create RTree")?;

    conn.execute(
        "CREATE VIRTUAL TABLE locations_fts USING fts5(
            nama, kecamatan, kota, provinsi, content='locations', content_rowid='id'
        )",
        [],
    )
    .ctx("failed to create FTS5")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS db_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )
    .ctx("failed to create db_meta table")?;

    conn.execute("CREATE INDEX idx_locations_nama ON locations(nama)", [])
        .ctx("failed to create nama index")?;
    conn.execute(
        "CREATE UNIQUE INDEX idx_locations_kode ON locations(kode)",
        [],
    )
    .ctx("failed to create kode index")?;

    Ok(())
}

fn insert_db_meta(
    conn: &Connection,
    decree: &str,
    source: &str,
    build_date: u64,
    village_count: usize,
) -> Result<(), PipelineError> {
    let mut ins_meta = conn
        .prepare("INSERT INTO db_meta (key, value) VALUES (?1, ?2)")
        .ctx("prepare insert db_meta")?;
    ins_meta
        .execute(rusqlite::params!["decree", decree])
        .ctx("insert db_meta decree")?;
    ins_meta
        .execute(rusqlite::params!["source", source])
        .ctx("insert db_meta source")?;
    ins_meta
        .execute(rusqlite::params!["build_date", build_date.to_string()])
        .ctx("insert db_meta build_date")?;
    ins_meta
        .execute(rusqlite::params![
            "village_count",
            village_count.to_string()
        ])
        .ctx("insert db_meta village_count")?;
    Ok(())
}

fn insert_villages(conn: &mut Connection, villages: &[MergedVillage]) -> Result<(), PipelineError> {
    let tx = conn.transaction().ctx("failed to begin transaction")?;
    {
        let mut ins_loc = tx
            .prepare(
                "INSERT INTO locations (id, kode, nama, kecamatan, kota, provinsi, lat, lon) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .ctx("prepare insert locations")?;
        let mut ins_rtree = tx
            .prepare(
                "INSERT INTO geo_rtree (id, min_lon, max_lon, min_lat, max_lat) VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .ctx("prepare insert rtree")?;

        for (i, v) in villages.iter().enumerate() {
            let rowid = (i + 1) as i64;
            ins_loc
                .execute(rusqlite::params![
                    rowid,
                    v.kode,
                    v.nama,
                    v.kecamatan,
                    v.kota,
                    v.provinsi,
                    v.lat,
                    v.lon
                ])
                .ctx("insert location")?;
            ins_rtree
                .execute(rusqlite::params![rowid, v.lon, v.lon, v.lat, v.lat])
                .ctx("insert rtree")?;
        }
    }
    tx.commit().ctx("failed to commit transaction")?;
    Ok(())
}

fn optimize_db(conn: &Connection) -> Result<(), PipelineError> {
    conn.execute_batch("PRAGMA analysis_limit = 400; PRAGMA optimize; VACUUM;")
        .ctx("optimize failed")?;
    Ok(())
}

/// Build the polygon database with vertex BLOBs and bbox indexes.
pub(crate) fn build_poly_db(
    big_data: &[BigRecord],
    poly_db_path: &Path,
    ring_classification: RingClassification,
) -> Result<(), PipelineError> {
    if poly_db_path.exists() {
        fs::remove_file(poly_db_path).ctx("failed to remove existing poly DB")?;
    }

    let mut conn = Connection::open(poly_db_path).ctx("failed to create poly DB")?;
    create_poly_db_schema(&conn)?;
    insert_polygons(&mut conn, big_data, ring_classification)?;
    optimize_db(&conn)?;

    let size = fs::metadata(poly_db_path).ctx("failed to get poly DB metadata")?;
    eprintln!(
        "Polygon database written: {:.1} MB",
        size.len() as f64 / (1024.0 * 1024.0)
    );

    Ok(())
}

fn create_poly_db_schema(conn: &Connection) -> Result<(), PipelineError> {
    conn.execute_batch(
        "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA page_size = 4096;",
    )
    .ctx("PRAGMA failed")?;

    conn.execute(
        "CREATE TABLE village_polygons (
            id INTEGER PRIMARY KEY,
            village_id INTEGER NOT NULL,
            ring_idx INTEGER NOT NULL,
            ring_type TEXT NOT NULL DEFAULT 'exterior',
            parent_ring_id INTEGER,
            min_lon REAL NOT NULL,
            max_lon REAL NOT NULL,
            min_lat REAL NOT NULL,
            max_lat REAL NOT NULL,
            vertices BLOB NOT NULL
        )",
        [],
    )
    .ctx("failed to create village_polygons table")?;

    conn.execute(
        "CREATE INDEX idx_vp_village ON village_polygons(village_id)",
        [],
    )
    .ctx("failed to create village index")?;
    conn.execute(
        "CREATE INDEX idx_vp_bbox ON village_polygons(min_lon, max_lon, min_lat, max_lat)",
        [],
    )
    .ctx("failed to create bbox index")?;

    Ok(())
}

fn insert_polygons(
    conn: &mut Connection,
    big_data: &[BigRecord],
    ring_classification: RingClassification,
) -> Result<(), PipelineError> {
    let tx = conn
        .transaction()
        .ctx("failed to begin poly DB transaction")?;
    {
        let mut ins = tx
            .prepare(
                "INSERT INTO village_polygons (id, village_id, ring_idx, ring_type, parent_ring_id, min_lon, max_lon, min_lat, max_lat, vertices) \
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .ctx("prepare insert village_polygons")?;

        let mut row_id: i64 = 0;
        for (village_idx, record) in big_data.iter().enumerate() {
            let rings = match &record.rings {
                Some(r) => r,
                None => continue,
            };

            let classified = match ring_classification {
                RingClassification::SeparateRings => {
                    rings.iter().map(|_| "exterior").collect::<Vec<_>>()
                }
                RingClassification::ClassifyHoles => geometry::classify_rings(rings),
            };

            for (ring_idx, ring) in rings.iter().enumerate() {
                if ring.len() < 3 {
                    continue;
                }

                let ring_type = classified[ring_idx];
                let vertices: Vec<(f64, f64)> = ring.iter().map(|&[lat, lon]| (lat, lon)).collect();
                let blob = crate::types::serialize_vertices(&vertices);

                let (min_lat, max_lat, min_lon, max_lon) = crate::types::bbox(&vertices);

                row_id += 1;
                ins.execute(rusqlite::params![
                    row_id,
                    village_idx as i64 + 1,
                    ring_idx as i64,
                    ring_type,
                    Option::<i64>::None,
                    min_lon,
                    max_lon,
                    min_lat,
                    max_lat,
                    blob,
                ])
                .ctx("insert village_polygon")?;
            }
        }
    }

    tx.commit().ctx("failed to commit poly DB transaction")?;
    Ok(())
}

/// Compute the SHA-256 hash of a file, returned as lowercase hex.
pub(crate) fn compute_sha256(db_path: &Path) -> Result<String, PipelineError> {
    let data = fs::read(db_path).ctx("failed to read DB for SHA-256")?;
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(&data);
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_big_record(code: &str, lat: f64, lon: f64) -> BigRecord {
        BigRecord {
            code: code.to_string(),
            name: code.to_string(),
            district: "Kemayoran".to_string(),
            city: "Jakarta Pusat".to_string(),
            province: "Jakarta".to_string(),
            lat,
            lon,
            rings: None,
        }
    }

    fn make_village(code: &str, district: &str, city: &str, province: &str) -> VillageRecord {
        VillageRecord {
            code: code.to_string(),
            name: code.to_string(),
            district: district.to_string(),
            city: city.to_string(),
            province: province.to_string(),
        }
    }

    #[test]
    fn test_merge_villages_match() {
        let villages = vec![make_village(
            "31.71.03.1001",
            "Kemayoran",
            "Jakarta Pusat",
            "Jakarta",
        )];
        let big_data = vec![make_big_record("31.71.03.1001", -6.1647, 106.8453)];
        let merged = merge_villages(&villages, &big_data);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].lat, -6.1647);
        assert_eq!(merged[0].lon, 106.8453);
    }

    #[test]
    fn test_merge_villages_fallback_kecamatan() {
        let villages = vec![make_village(
            "31.71.03.1002",
            "Kemayoran",
            "Jakarta Pusat",
            "Jakarta",
        )];
        let big_data = vec![make_big_record("31.71.03.1001", -6.1647, 106.8453)];
        let merged = merge_villages(&villages, &big_data);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].lat, -6.1647);
        assert_eq!(merged[0].lon, 106.8453);
    }

    #[test]
    fn test_merge_villages_fallback_no_kecamatan() {
        let villages = vec![make_village(
            "99.99.99.9999",
            "Unknown",
            "Unknown City",
            "Unknown Province",
        )];
        let big_data = vec![];
        let merged = merge_villages(&villages, &big_data);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].lat, 0.0);
        assert_eq!(merged[0].lon, 0.0);
    }

    #[test]
    fn test_build_db_creates_valid_sqlite() {
        let villages = vec![
            MergedVillage {
                kode: "31.71.03.1001".to_string(),
                nama: "Kemayoran".to_string(),
                kecamatan: "Kemayoran".to_string(),
                kota: "Jakarta Pusat".to_string(),
                provinsi: "Jakarta".to_string(),
                lat: -6.1647,
                lon: 106.8453,
            },
            MergedVillage {
                kode: "31.71.03.1002".to_string(),
                nama: "Gelora".to_string(),
                kecamatan: "Senayan".to_string(),
                kota: "Jakarta Selatan".to_string(),
                provinsi: "Jakarta".to_string(),
                lat: -6.1600,
                lon: 106.8500,
            },
        ];

        let temp_dir = std::env::temp_dir();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let db_path = temp_dir.join(format!("test_wilayah_{}.db", timestamp));

        build_db(&villages, &db_path, "Test Decree", "test", 1234567890)
            .expect("build_db should succeed");

        let conn = rusqlite::Connection::open(&db_path).expect("open built DB");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM locations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let rtree_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM geo_rtree", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rtree_count, 2);

        let decree: String = conn
            .query_row(
                "SELECT value FROM db_meta WHERE key = 'decree'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(decree, "Test Decree");

        let source: String = conn
            .query_row(
                "SELECT value FROM db_meta WHERE key = 'source'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(source, "test");

        let build_date: String = conn
            .query_row(
                "SELECT value FROM db_meta WHERE key = 'build_date'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(build_date, "1234567890");

        let village_count_meta: String = conn
            .query_row(
                "SELECT value FROM db_meta WHERE key = 'village_count'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(village_count_meta, "2");

        let mut stmt = conn
            .prepare(
                "SELECT l.kode FROM locations_fts f \
                JOIN locations l ON f.rowid = l.id \
                WHERE locations_fts MATCH 'Kemayoran'",
            )
            .unwrap();
        let rows = stmt.query_map([], |row| row.get::<_, String>(0)).unwrap();
        let results: Vec<String> = rows.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "31.71.03.1001");

        fs::remove_file(&db_path).unwrap();
    }
}
