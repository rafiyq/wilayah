//! Shared test utilities.
//!
//! The `village_polygons` CREATE TABLE and index DDL below duplicates the schema
//! in `src/builder/db_create.rs::create_poly_db_schema`. Keep them in sync when
//! altering the polygon DB schema (e.g., column additions/removals).

fn bbox_inline(ring: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let mut min_lat = f64::MAX;
    let mut max_lat = f64::MIN;
    let mut min_lon = f64::MAX;
    let mut max_lon = f64::MIN;
    for &(lat, lon) in ring {
        min_lat = min_lat.min(lat);
        max_lat = max_lat.max(lat);
        min_lon = min_lon.min(lon);
        max_lon = max_lon.max(lon);
    }
    (min_lat, max_lat, min_lon, max_lon)
}

fn serialize_vertices_inline(ring: &[(f64, f64)]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(ring.len() * 16);
    for &(lat, lon) in ring {
        buf.extend_from_slice(&lat.to_le_bytes());
        buf.extend_from_slice(&lon.to_le_bytes());
    }
    buf
}

pub fn create_test_poly_db(village_id: i64, ring: &[(f64, f64)]) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().expect("create temp file");
    let conn = rusqlite::Connection::open(file.path()).expect("open poly DB");

    conn.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;")
        .unwrap();

    conn.execute(
        "CREATE TABLE village_polygons (\
         id INTEGER PRIMARY KEY, \
         village_id INTEGER NOT NULL, \
         ring_idx INTEGER NOT NULL, \
         ring_type TEXT NOT NULL DEFAULT 'exterior', \
         min_lon REAL NOT NULL, \
         max_lon REAL NOT NULL, \
         min_lat REAL NOT NULL, \
         max_lat REAL NOT NULL, \
         vertices BLOB NOT NULL\
         )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE INDEX idx_vp_village ON village_polygons(village_id)",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE INDEX idx_vp_bbox ON village_polygons(min_lon, max_lon, min_lat, max_lat)",
        [],
    )
    .unwrap();

    let (min_lat, max_lat, min_lon, max_lon) = bbox_inline(ring);

    let blob = serialize_vertices_inline(ring);

    conn.execute(
        "INSERT INTO village_polygons (id, village_id, ring_idx, ring_type, min_lon, max_lon, min_lat, max_lat, vertices) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![1, village_id, 0, "exterior", min_lon, max_lon, min_lat, max_lat, blob],
    )
    .unwrap();

    drop(conn);
    file
}
