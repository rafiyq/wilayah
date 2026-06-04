use wilayah::{Database, LookupResult, NEAREST_MAX_LIMIT, SEARCH_MAX_LIMIT};

#[test]
fn test_open_db() {
    let db = Database::open().expect("should open embedded database");
    let count = db.village_count().expect("should count villages");
    assert!(count > 80000, "expected >80k villages, got {count}");
}

#[test]
fn test_data_info() {
    let info = wilayah::data_info();
    if info.village_count > 0 {
        assert!(info.village_count > 80000);
    }
    if info.build_date > 0 {
        assert!(!info.source.is_empty());
        assert!(!info.decree.is_empty());
        assert!(
            !info.decree.contains("unknown"),
            "decree should be from DB, not 'unknown': {}",
            info.decree
        );
    }
}

#[test]
fn test_data_info_via_database() {
    let db = Database::open().unwrap();
    let info = db.data_info();
    assert_eq!(info, wilayah::data_info());
}

#[test]
fn test_nearest_jakarta() {
    let db = Database::open().unwrap();
    let results = db.find_nearest(-6.1647, 106.8453, 1).unwrap();
    assert_eq!(results.len(), 1);
    let v = &results[0];
    assert!(
        v.dist_km.unwrap() < 5.0,
        "should be within 5km of Jakarta center"
    );
    assert_eq!(
        v.city, "Kota Administrasi Jakarta Pusat",
        "expected Jakarta Pusat, got {}",
        v.city
    );
}

#[test]
fn test_nearest_papua() {
    let db = Database::open().unwrap();
    let results = db.find_nearest(-2.5, 140.0, 1).unwrap();
    assert!(!results.is_empty());
    assert!(results[0].province.contains("Papua"));
}

#[test]
fn test_search() {
    let db = Database::open().unwrap();
    let results = db.find_by_name("kemayoran", 5).unwrap();
    assert!(!results.is_empty(), "should find Kemayoran");
    assert!(results
        .iter()
        .any(|v| v.name.to_lowercase().contains("kemayoran")));
}

#[test]
fn test_search_qualified() {
    let db = Database::open().unwrap();
    let results = db.find_by_name("kemayoran jakarta", 5).unwrap();
    assert!(!results.is_empty(), "should find Kemayoran Jakarta");
    assert!(results.iter().all(|v| v.city.contains("Jakarta")));
}

#[test]
fn test_unique_found() {
    let db = Database::open().unwrap();
    let result = db.find_by_name_unique("abadijaya").unwrap();
    assert!(
        matches!(result, LookupResult::Found(_)),
        "expected Found, got {:?}",
        result
    );
    if let LookupResult::Found(v) = result {
        assert_eq!(v.name, "Abadijaya");
    }
}

#[test]
fn test_unique_ambiguous() {
    let db = Database::open().unwrap();
    let result = db.find_by_name_unique("sukamaju").unwrap();
    assert!(
        matches!(result, LookupResult::Ambiguous(_)),
        "sukamaju should be ambiguous, got {:?}",
        result
    );
    if let LookupResult::Ambiguous(results) = result {
        assert!(results.len() > 1, "should have multiple matches");
    }
}

#[test]
fn test_unique_not_found() {
    let db = Database::open().unwrap();
    let result = db.find_by_name_unique("zzzznonexistent").unwrap();
    assert!(
        matches!(result, LookupResult::NotFound),
        "should be not found, got {:?}",
        result
    );
}

#[test]
fn test_find_by_code() {
    let db = Database::open().unwrap();
    let v = db.find_by_code("31.71.03.1001").unwrap();
    assert!(v.is_some(), "31.71.03.1001 should exist");
    let v = v.unwrap();
    assert_eq!(v.name, "Kemayoran");
    assert_eq!(v.district, "Kemayoran");
    assert_eq!(v.city, "Kota Administrasi Jakarta Pusat");
    assert_eq!(v.province, "Provinsi Daerah Khusus Ibukota Jakarta");
}

#[test]
fn test_find_by_code_not_found() {
    let db = Database::open().unwrap();
    let v = db.find_by_code("99.99.99.9999").unwrap();
    assert!(v.is_none());
}

#[test]
fn test_find_by_code_prefix_kecamatan() {
    let db = Database::open().unwrap();
    let result = db.find_by_code_prefix("31.71.03", 100, 0).unwrap();
    assert!(
        !result.villages.is_empty(),
        "should find villages in kecamatan 31.71.03"
    );
    assert!(result
        .villages
        .iter()
        .all(|v| v.code.starts_with("31.71.03")));
    assert!(result.villages.iter().all(|v| v.district == "Kemayoran"));
    assert_eq!(result.total, result.villages.len());
    assert!(!result.has_more);
}

#[test]
fn test_find_by_code_prefix_kabupaten() {
    let db = Database::open().unwrap();
    let result = db.find_by_code_prefix("31.71", 500, 0).unwrap();
    assert!(
        !result.villages.is_empty(),
        "should find villages in kabupaten 31.71"
    );
    assert!(result.villages.iter().all(|v| v.code.starts_with("31.71")));
    assert!(result.total > 0);
    assert_eq!(result.has_more, result.villages.len() < result.total);
}

#[test]
fn test_find_by_code_prefix_not_found() {
    let db = Database::open().unwrap();
    let result = db.find_by_code_prefix("99.99.99", 100, 0).unwrap();
    assert!(result.villages.is_empty());
    assert_eq!(result.total, 0);
    assert!(!result.has_more);
}

#[test]
fn test_find_nearest_limit_zero() {
    let db = Database::open().unwrap();
    let results = db.find_nearest(-6.1647, 106.8453, 0).unwrap();
    assert_eq!(results.len(), 1, "limit=0 should clamp to 1");
}

#[test]
fn test_find_nearest_limit_exceeds_max() {
    let db = Database::open().unwrap();
    let results = db.find_nearest(-6.1647, 106.8453, 99999).unwrap();
    assert!(
        results.len() <= NEAREST_MAX_LIMIT,
        "limit=99999 should clamp to {}, got {}",
        NEAREST_MAX_LIMIT,
        results.len()
    );
}

#[test]
fn test_find_by_name_limit_zero() {
    let db = Database::open().unwrap();
    let results = db.find_by_name("kemayoran", 0).unwrap();
    assert!(
        results.len() <= 1,
        "limit=0 should clamp to 1, got {}",
        results.len()
    );
}

#[test]
fn test_find_by_name_limit_exceeds_max() {
    let db = Database::open().unwrap();
    let results = db.find_by_name("kemayoran", 99999).unwrap();
    assert!(
        results.len() <= SEARCH_MAX_LIMIT,
        "limit=99999 should clamp to {}, got {}",
        SEARCH_MAX_LIMIT,
        results.len()
    );
}

#[test]
fn test_find_by_code_prefix_offset() {
    let db = Database::open().unwrap();
    let page1 = db.find_by_code_prefix("31.71", 5, 0).unwrap();
    let page2 = db.find_by_code_prefix("31.71", 5, 5).unwrap();
    assert!(!page1.villages.is_empty());
    assert!(!page2.villages.is_empty());
    assert!(page1.villages[0].code != page2.villages[0].code);
}

#[test]
fn test_find_by_code_prefix_offset_beyond_total() {
    let db = Database::open().unwrap();
    let result = db.find_by_code_prefix("31.71.03", 100, 99999).unwrap();
    assert!(
        result.villages.is_empty(),
        "offset beyond total should return empty"
    );
    assert!(result.total > 0);
    assert!(!result.has_more);
}

#[test]
fn test_find_nearest_sorted_by_distance() {
    let db = Database::open().unwrap();
    let results = db.find_nearest(-6.1647, 106.8453, 5).unwrap();
    for i in 1..results.len() {
        assert!(
            results[i - 1].dist_km <= results[i].dist_km,
            "results not sorted by distance: {} > {} at index {}",
            results[i - 1].dist_km.unwrap(),
            results[i].dist_km.unwrap(),
            i
        );
    }
}
