use regex::Regex;
use wilayah::{
    data_info_from_conn, find_by_code, find_by_code_prefix, find_by_name, open, village_count,
};

#[test]
fn test_db_has_many_villages() {
    let conn = open().expect("open embedded DB");
    let count = village_count(&conn).expect("count villages");
    assert!(count > 80_000, "expected >80k villages, got {}", count);
}

#[test]
fn test_all_codes_valid_format() {
    let conn = open().unwrap();
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
    let conn = open().unwrap();
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
    let conn = open().unwrap();
    let mut stmt = conn
        .prepare("SELECT kode, lat, lon FROM locations")
        .unwrap();
    let mut rows = stmt.query([]).unwrap();
    let mut out_of_bounds = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        let code: String = row.get(0).unwrap();
        let lat: f64 = row.get(1).unwrap();
        let lon: f64 = row.get(2).unwrap();
        // Allow (0,0) as special fallback for villages without coordinates.
        if lat == 0.0 && lon == 0.0 {
            continue;
        }
        // Approximate bounds of Indonesia
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
    let conn = open().unwrap();
    let loc_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM locations", [], |row| row.get(0))
        .unwrap();
    let rtree_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM geo_rtree", [], |row| row.get(0))
        .unwrap();
    assert_eq!(loc_count, rtree_count);
}

#[test]
fn test_fts_search_works() {
    let conn = open().unwrap();
    // Search for a known common village name; should return something.
    let results = find_by_name(&conn, "kemayoran", 10).unwrap();
    assert!(!results.is_empty());
    assert!(results
        .iter()
        .any(|v| v.name.to_lowercase().contains("kemayoran")));
}

#[test]
fn test_find_by_code_known() {
    let conn = open().unwrap();
    let v = find_by_code(&conn, "31.71.03.1001").unwrap();
    assert!(v.is_some());
    let v = v.unwrap();
    assert_eq!(v.code, "31.71.03.1001");
    assert_eq!(v.name, "Kemayoran");
}

#[test]
fn test_find_by_code_prefix_province() {
    let conn = open().unwrap();
    // Prefix "31" should return many villages (all in DKI Jakarta province)
    let result = find_by_code_prefix(&conn, "31", 100, 0).unwrap();
    assert!(!result.villages.is_empty());
    assert!(result.villages.iter().all(|v| v.code.starts_with("31")));
    // Province-level query should exceed the limit, demonstrating pagination need
    assert!(result.total > 100);
    assert!(result.has_more);
}

#[test]
fn test_db_meta_table() {
    let conn = open().unwrap();
    let info = data_info_from_conn(&conn);
    // If db_meta exists (pipeline-rebuilt DB), verify values
    if info.village_count > 0 && info.build_date > 0 {
        assert!(
            !info.decree.contains("unknown"),
            "decree should not be 'unknown', got: {}",
            info.decree
        );
        assert!(!info.source.is_empty(), "source should not be empty");
    }
}
