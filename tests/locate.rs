use wilayah::{Database, LocateMethod};

#[test]
fn test_locate_jakarta() {
    let db = Database::open().unwrap();
    let loc = db
        .locate(-6.1647, 106.8453)
        .unwrap()
        .expect("should locate Jakarta");
    assert_eq!(loc.province.code, "31");
    assert!(loc.city.name.contains("Jakarta"));
    assert!(loc.district.name.len() > 0);
    assert!(loc.village.len() > 0);
    assert!(loc.village_code.contains('.'));
    assert!(loc.dist_km < 5.0);
    assert_eq!(loc.method, LocateMethod::Nearest);
}

#[test]
fn test_locate_display() {
    let db = Database::open().unwrap();
    let loc = db
        .locate(-6.1647, 106.8453)
        .unwrap()
        .expect("should locate Jakarta");
    let s = format!("{loc}");
    assert!(s.contains(&loc.province.code));
    assert!(s.contains(&loc.village));
}

#[test]
fn test_open_with_polygons_invalid_path() {
    let result = Database::open_with_polygons("/nonexistent/path/poly.db");
    assert!(result.is_err(), "invalid poly path should return error");
}
