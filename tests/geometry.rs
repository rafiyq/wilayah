use wilayah::{
    bbox, deserialize_vertices, haversine_km, point_in_polygon, point_in_ring, serialize_vertices,
    PipResult,
};

#[test]
fn test_pip_square_inside() {
    let square = [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
    assert_eq!(point_in_ring(0.5, 0.5, &square), PipResult::Inside);
}

#[test]
fn test_pip_square_outside() {
    let square = [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
    assert_eq!(point_in_ring(1.5, 0.5, &square), PipResult::Outside);
}

#[test]
fn test_pip_square_on_edge() {
    let square = [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
    assert_eq!(point_in_ring(0.5, 0.0, &square), PipResult::OnBoundary);
}

#[test]
fn test_pip_square_on_vertex() {
    let square = [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
    assert_eq!(point_in_ring(0.0, 0.0, &square), PipResult::OnBoundary);
}

#[test]
fn test_pip_triangle_vertex() {
    let tri = [(0.0, 0.0), (0.5, 1.0), (1.0, 0.0), (0.0, 0.0)];
    assert_eq!(point_in_ring(0.5, 1.0, &tri), PipResult::OnBoundary);
}

#[test]
fn test_pip_with_hole_inside() {
    let exterior = [(0.0, 0.0), (0.0, 4.0), (4.0, 4.0), (4.0, 0.0), (0.0, 0.0)];
    let hole = [(1.0, 1.0), (1.0, 2.0), (2.0, 2.0), (2.0, 1.0), (1.0, 1.0)];
    assert!(point_in_polygon(3.0, 3.0, &exterior, &[&hole]));
}

#[test]
fn test_pip_with_hole_in_hole() {
    let exterior = [(0.0, 0.0), (0.0, 4.0), (4.0, 4.0), (4.0, 0.0), (0.0, 0.0)];
    let hole = [(1.0, 1.0), (1.0, 2.0), (2.0, 2.0), (2.0, 1.0), (1.0, 1.0)];
    assert!(!point_in_polygon(1.5, 1.5, &exterior, &[&hole]));
}

#[test]
fn test_pip_with_hole_on_hole_boundary() {
    let exterior = [(0.0, 0.0), (0.0, 4.0), (4.0, 4.0), (4.0, 0.0), (0.0, 0.0)];
    let hole = [(1.0, 1.0), (1.0, 2.0), (2.0, 2.0), (2.0, 1.0), (1.0, 1.0)];
    assert!(!point_in_polygon(1.5, 1.0, &exterior, &[&hole]));
}

#[test]
fn test_pip_concave() {
    let concave = [
        (0.0, 0.0),
        (0.0, 3.0),
        (2.0, 3.0),
        (2.0, 2.0),
        (1.0, 2.0),
        (1.0, 1.0),
        (2.0, 1.0),
        (2.0, 0.0),
        (0.0, 0.0),
    ];
    assert_eq!(point_in_ring(0.5, 0.5, &concave), PipResult::Inside);
    assert_eq!(point_in_ring(1.5, 0.5, &concave), PipResult::Inside);
    assert_eq!(point_in_ring(1.5, 1.5, &concave), PipResult::Outside);
    assert_eq!(point_in_ring(1.5, 2.5, &concave), PipResult::Inside);
}

#[test]
fn test_pip_horizontal_edge_on_boundary() {
    let square = [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
    assert_eq!(
        point_in_ring(0.5, 0.0, &square),
        PipResult::OnBoundary,
        "point on bottom horizontal edge should be OnBoundary"
    );
    assert_eq!(
        point_in_ring(0.5, 1.0, &square),
        PipResult::OnBoundary,
        "point on top horizontal edge should be OnBoundary"
    );
}

#[test]
fn test_pip_no_rings() {
    let empty: [(f64, f64); 0] = [];
    assert_eq!(point_in_ring(0.5, 0.5, &empty), PipResult::Outside);
}

#[test]
fn test_pip_degenerate_two_points() {
    let degenerate = [(0.0, 0.0), (1.0, 1.0)];
    assert_eq!(point_in_ring(0.5, 0.5, &degenerate), PipResult::Outside);
}

#[test]
fn test_serialize_deserialize_vertices() {
    let ring = vec![(-6.16, 106.85), (-6.17, 106.86), (-6.18, 106.87)];
    let blob = serialize_vertices(&ring);
    assert_eq!(blob.len(), 3 * 16);
    let restored = deserialize_vertices(&blob);
    assert_eq!(restored, ring);
}

#[test]
fn test_bbox() {
    let ring = vec![(-6.0, 106.0), (-8.0, 108.0), (-7.0, 107.0)];
    let (min_lat, max_lat, min_lon, max_lon) = bbox(&ring);
    assert_eq!(min_lat, -8.0);
    assert_eq!(max_lat, -6.0);
    assert_eq!(min_lon, 106.0);
    assert_eq!(max_lon, 108.0);
}

#[test]
fn test_haversine_km() {
    let d = haversine_km(-6.1647, 106.8453, -6.1647, 106.8453);
    assert!(d.abs() < 0.001, "same point should be 0 km, got {d}");
    let d = haversine_km(-6.1647, 106.8453, -6.2, 106.8);
    assert!(d > 0.0 && d < 50.0, "nearby point should be close, got {d}");
}

#[test]
fn test_haversine_known_distance() {
    let jakarta_lat = -6.2088;
    let jakarta_lon = 106.8456;
    let bandung_lat = -6.9175;
    let bandung_lon = 107.6191;
    let d = haversine_km(jakarta_lat, jakarta_lon, bandung_lat, bandung_lon);
    assert!(
        (d - 120.0).abs() < 10.0,
        "Jakarta-Bandung should be ~120 km, got {d}"
    );
}

#[test]
fn test_haversine_symmetric() {
    let d1 = haversine_km(-6.1647, 106.8453, -6.9175, 107.6191);
    let d2 = haversine_km(-6.9175, 107.6191, -6.1647, 106.8453);
    assert!(
        (d1 - d2).abs() < 1e-10,
        "haversine should be symmetric: {d1} vs {d2}"
    );
}

#[test]
fn test_haversine_antipodal() {
    let d = haversine_km(0.0, 0.0, 0.0, 180.0);
    let expected = 6371.0 * std::f64::consts::PI;
    assert!(
        (d - expected).abs() < 1.0,
        "antipodal distance should be ~{expected:.0} km, got {d}"
    );
}

#[test]
fn test_deserialize_vertices_empty() {
    let blob = [];
    let ring = deserialize_vertices(&blob);
    assert!(ring.is_empty(), "empty blob should yield empty vec");
}

#[test]
fn test_deserialize_vertices_trailing() {
    let ring = vec![(-6.16, 106.85)];
    let mut blob = serialize_vertices(&ring);
    blob.push(0xFF);
    let restored = deserialize_vertices(&blob);
    assert_eq!(restored.len(), 1, "trailing byte should be ignored");
    assert_eq!(restored[0], (-6.16, 106.85));
}

#[test]
fn test_bbox_single_point() {
    let ring = vec![(-6.0, 106.0)];
    let (min_lat, max_lat, min_lon, max_lon) = bbox(&ring);
    assert_eq!(min_lat, -6.0);
    assert_eq!(max_lat, -6.0);
    assert_eq!(min_lon, 106.0);
    assert_eq!(max_lon, 106.0);
}

#[test]
fn test_bbox_identical_points() {
    let ring = vec![(-6.0, 106.0), (-6.0, 106.0), (-6.0, 106.0)];
    let (min_lat, max_lat, min_lon, max_lon) = bbox(&ring);
    assert_eq!(min_lat, max_lat);
    assert_eq!(min_lon, max_lon);
}

#[test]
fn test_point_in_polygon_no_interiors() {
    let exterior = [(0.0, 0.0), (0.0, 4.0), (4.0, 4.0), (4.0, 0.0), (0.0, 0.0)];
    assert!(
        point_in_polygon(2.0, 2.0, &exterior, &[]),
        "point inside exterior with no interiors should be inside"
    );
    assert!(
        !point_in_polygon(5.0, 5.0, &exterior, &[]),
        "point outside exterior with no interiors should be outside"
    );
}

#[test]
fn test_point_in_polygon_multiple_holes() {
    let exterior = [
        (0.0, 0.0),
        (0.0, 10.0),
        (10.0, 10.0),
        (10.0, 0.0),
        (0.0, 0.0),
    ];
    let hole1 = [(1.0, 1.0), (1.0, 3.0), (3.0, 3.0), (3.0, 1.0), (1.0, 1.0)];
    let hole2 = [(7.0, 7.0), (7.0, 9.0), (9.0, 9.0), (9.0, 7.0), (7.0, 7.0)];
    assert!(
        point_in_polygon(5.0, 5.0, &exterior, &[&hole1, &hole2]),
        "point between two holes should be inside"
    );
    assert!(
        !point_in_polygon(2.0, 2.0, &exterior, &[&hole1, &hole2]),
        "point in first hole should be outside"
    );
    assert!(
        !point_in_polygon(8.0, 8.0, &exterior, &[&hole1, &hole2]),
        "point in second hole should be outside"
    );
}
