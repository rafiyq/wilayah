use wilayah::{location_from_village, Database, LocateMethod, Village};

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
fn test_locate_method_display() {
    assert_eq!(format!("{}", LocateMethod::Nearest), "nearest");
    assert_eq!(format!("{}", LocateMethod::Contained), "contained");
}

#[test]
fn test_location_from_village() {
    let v = Village {
        code: "31.71.03.1001".into(),
        name: "Kemayoran".into(),
        district: "Kemayoran".into(),
        city: "Jakarta Pusat".into(),
        province: "DKI Jakarta".into(),
        lat: -6.1647,
        lon: 106.8453,
        dist_km: None,
    };
    let loc =
        location_from_village(&v, 1.5, LocateMethod::Nearest).expect("should parse valid code");
    assert_eq!(loc.province.code, "31");
    assert_eq!(loc.city.code, "31.71");
    assert_eq!(loc.district.code, "31.71.03");
    assert_eq!(loc.village_code, "31.71.03.1001");
    assert_eq!(loc.dist_km, 1.5);
    assert_eq!(loc.method, LocateMethod::Nearest);
}

#[test]
fn test_location_from_village_bad_code() {
    let v = Village {
        code: "invalid".into(),
        name: "Test".into(),
        district: "Test".into(),
        city: "Test".into(),
        province: "Test".into(),
        lat: 0.0,
        lon: 0.0,
        dist_km: None,
    };
    assert!(location_from_village(&v, 0.0, LocateMethod::Nearest).is_none());
}

#[test]
fn test_location_from_village_three_parts() {
    let v = Village {
        code: "31.71.03".into(),
        name: "Test".into(),
        district: "Test".into(),
        city: "Test".into(),
        province: "Test".into(),
        lat: 0.0,
        lon: 0.0,
        dist_km: None,
    };
    assert!(
        location_from_village(&v, 0.0, LocateMethod::Nearest).is_none(),
        "3-part code should return None"
    );
}

#[test]
fn test_location_from_village_five_parts() {
    let v = Village {
        code: "31.71.03.1001.5".into(),
        name: "Test".into(),
        district: "Test".into(),
        city: "Test".into(),
        province: "Test".into(),
        lat: 0.0,
        lon: 0.0,
        dist_km: None,
    };
    assert!(
        location_from_village(&v, 0.0, LocateMethod::Nearest).is_none(),
        "5-part code should return None"
    );
}

#[test]
fn test_open_with_polygons_invalid_path() {
    let result = Database::open_with_polygons("/nonexistent/path/poly.db");
    assert!(result.is_err(), "invalid poly path should return error");
}
