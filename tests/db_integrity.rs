use regex::Regex;
use wilayah::Database;

#[test]
fn test_db_has_many_villages() {
    let db = Database::open().expect("open embedded DB");
    let count = db.village_count().expect("count villages");
    assert!(count > 80_000, "expected >80k villages, got {}", count);
}

#[test]
fn test_all_codes_valid_format() {
    let db = Database::open().unwrap();
    let conn = db.conn_guard();
    let mut stmt = conn.prepare("SELECT kode FROM locations").unwrap();
    let mut rows = stmt.query([]).unwrap();
    let re = Regex::new(r"^\d{2}\.\d{2}\.\d{2}\.\d{4}$").unwrap();
    let mut bad_codes = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        let code: String = row.get(0).unwrap();
        if !re.is_match(&code) {
            bad_codes.push(code);
        }
    }
    assert!(bad_codes.is_empty(), "invalid codes: {:?}", bad_codes);
}

#[test]
fn test_no_duplicate_codes() {
    let db = Database::open().unwrap();
    let conn = db.conn_guard();
    let mut stmt = conn
        .prepare("SELECT kode, COUNT(*) FROM locations GROUP BY kode HAVING COUNT(*) > 1")
        .unwrap();
    let mut rows = stmt.query([]).unwrap();
    let mut duplicates = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        let code: String = row.get(0).unwrap();
        let count: i64 = row.get(1).unwrap();
        duplicates.push((code, count));
    }
    assert!(duplicates.is_empty(), "duplicate codes: {:?}", duplicates);
}

#[test]
fn test_coordinates_within_bounds() {
    let db = Database::open().unwrap();
    let conn = db.conn_guard();
    let mut stmt = conn
        .prepare("SELECT kode, lat, lon FROM locations")
        .unwrap();
    let mut rows = stmt.query([]).unwrap();
    let mut out_of_bounds = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        let code: String = row.get(0).unwrap();
        let lat: f64 = row.get(1).unwrap();
        let lon: f64 = row.get(2).unwrap();
        if lat == 0.0 && lon == 0.0 {
            continue;
        }
        if !(-11.0..=6.0).contains(&lat) || !(95.0..=141.0).contains(&lon) {
            out_of_bounds.push((code, lat, lon));
        }
    }
    assert!(
        out_of_bounds.is_empty(),
        "coordinates out of bounds: {:?}",
        out_of_bounds
    );
}

#[test]
fn test_rtree_matches_locations_count() {
    let db = Database::open().unwrap();
    let conn = db.conn_guard();
    let loc_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM locations", [], |row| row.get(0))
        .unwrap();
    let rtree_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM geo_rtree", [], |row| row.get(0))
        .unwrap();
    assert_eq!(loc_count, rtree_count);
}

#[test]
fn test_db_meta_table() {
    let db = Database::open().unwrap();
    let info = db.data_info();
    if info.village_count > 0 && info.build_date > 0 {
        assert!(
            !info.decree.contains("unknown"),
            "decree should not be 'unknown', got: {}",
            info.decree
        );
        assert!(!info.source.is_empty(), "source should not be empty");
    }
}
