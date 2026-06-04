mod common;

use common::create_test_poly_db;
use wilayah::{Database, LocateMethod};

#[test]
fn test_locate_contained_inside_polygon() {
    let db = Database::open().expect("open embedded DB");
    let conn = db.conn_guard();

    let (code, vlat, vlon): (String, f64, f64) = conn
        .query_row(
            "SELECT kode, lat, lon FROM locations WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("village id=1 should exist");

    if vlat == 0.0 && vlon == 0.0 {
        eprintln!("Skipping: village id=1 has (0,0) coordinates");
        return;
    }

    drop(conn);

    let ring = vec![
        (vlat - 0.01, vlon - 0.01),
        (vlat - 0.01, vlon + 0.01),
        (vlat + 0.01, vlon + 0.01),
        (vlat + 0.01, vlon - 0.01),
        (vlat - 0.01, vlon - 0.01),
    ];

    let poly_file = create_test_poly_db(1, &ring);
    let path = poly_file.path().to_str().expect("temp path to str");

    let db = Database::open_with_polygons(path).expect("open with polygons");
    assert!(db.has_polygons());

    let loc = db
        .locate(vlat, vlon)
        .expect("locate")
        .expect("should find a location");
    assert_eq!(
        loc.method,
        LocateMethod::Contained,
        "point at village centroid should be contained, got {:?}",
        loc.method
    );
    assert_eq!(loc.village_code, code);
}

#[test]
fn test_locate_nearest_fallback_outside_polygon() {
    let db = Database::open().expect("open embedded DB");
    let conn = db.conn_guard();

    let (vlat, vlon): (f64, f64) = conn
        .query_row("SELECT lat, lon FROM locations WHERE id = 1", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .expect("village id=1 should exist");

    if vlat == 0.0 && vlon == 0.0 {
        eprintln!("Skipping: village id=1 has (0,0) coordinates");
        return;
    }

    drop(conn);

    let ring = vec![
        (vlat - 0.001, vlon - 0.001),
        (vlat - 0.001, vlon + 0.001),
        (vlat + 0.001, vlon + 0.001),
        (vlat + 0.001, vlon - 0.001),
        (vlat - 0.001, vlon - 0.001),
    ];

    let poly_file = create_test_poly_db(1, &ring);
    let path = poly_file.path().to_str().expect("temp path to str");

    let db = Database::open_with_polygons(path).expect("open with polygons");

    let far_lat = vlat + 0.1;
    let far_lon = vlon + 0.1;
    let loc = db
        .locate(far_lat, far_lon)
        .expect("locate")
        .expect("should find a location");
    assert_eq!(
        loc.method,
        LocateMethod::Nearest,
        "point far outside polygon should use nearest, got {:?}",
        loc.method
    );
}

#[test]
fn test_locate_without_polygons_is_nearest() {
    let db = Database::open().expect("open embedded DB");
    assert!(!db.has_polygons());

    let loc = db
        .locate(-6.1647, 106.8453)
        .expect("locate")
        .expect("should find a location");
    assert_eq!(loc.method, LocateMethod::Nearest);
}
