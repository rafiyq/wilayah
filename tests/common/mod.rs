use wilayah::{bbox, serialize_vertices};

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
         parent_ring_id INTEGER, \
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

    let (min_lat, max_lat, min_lon, max_lon) = bbox(ring);

    let blob = serialize_vertices(ring);

    conn.execute(
        "INSERT INTO village_polygons (id, village_id, ring_idx, ring_type, parent_ring_id, min_lon, max_lon, min_lat, max_lat, vertices) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![1, village_id, 0, "exterior", Option::<i64>::None, min_lon, max_lon, min_lat, max_lat, blob],
    )
    .unwrap();

    drop(conn);
    file
}
