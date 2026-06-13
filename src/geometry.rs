//! Geographic computations: distance, point-in-polygon, and vertex serialization.

/// Earth's mean radius in kilometers, used by [`haversine_km`].
pub const EARTH_RADIUS_KM: f64 = 6371.0;

/// Epsilon tolerance for floating-point comparisons in PIP calculations.
const PIP_EPSILON: f64 = 1e-10;

/// Compute the great-circle distance between two points using the Haversine formula.
///
/// Returns distance in kilometers.
pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    EARTH_RADIUS_KM * 2.0 * a.sqrt().asin()
}

/// Compute the axis-aligned bounding box of a polygon ring.
pub(crate) fn bbox(ring: &[(f64, f64)]) -> (f64, f64, f64, f64) {
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

/// Result of a point-in-polygon test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PipResult {
    /// The point is inside the polygon.
    Inside,
    /// The point is outside the polygon.
    Outside,
    /// The point lies exactly on a polygon edge or vertex.
    OnBoundary,
}

fn point_on_segment(px: f64, py: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> bool {
    let cross = (py - y1) * (x2 - x1) - (px - x1) * (y2 - y1);
    if cross.abs() > PIP_EPSILON {
        return false;
    }
    let (min_x, max_x) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
    let (min_y, max_y) = if y1 < y2 { (y1, y2) } else { (y2, y1) };
    px >= min_x - PIP_EPSILON
        && px <= max_x + PIP_EPSILON
        && py >= min_y - PIP_EPSILON
        && py <= max_y + PIP_EPSILON
}

fn ray_cast(px: f64, py: f64, ring: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let (x1, y1) = ring[i];
        let (x2, y2) = ring[(i + 1) % n];
        if (y1 > py) != (y2 > py) && px < (x2 - x1) * (py - y1) / (y2 - y1) + x1 {
            inside = !inside;
        }
    }
    inside
}

/// Test whether a point lies inside, outside, or on the boundary of a polygon ring.
///
/// The ring must be closed (first point equals last point). Uses the ray-casting
/// algorithm with the "upward exclusive, downward inclusive" rule to handle vertex
/// edge cases. A separate on-boundary check catches points that lie exactly on an
/// edge or vertex.
pub fn point_in_ring(px: f64, py: f64, ring: &[(f64, f64)]) -> PipResult {
    let n = ring.len();
    if n < 3 {
        return PipResult::Outside;
    }
    for i in 0..n {
        let (x1, y1) = ring[i];
        let (x2, y2) = ring[(i + 1) % n];
        if point_on_segment(px, py, x1, y1, x2, y2) {
            return PipResult::OnBoundary;
        }
    }
    if ray_cast(px, py, ring) {
        PipResult::Inside
    } else {
        PipResult::Outside
    }
}

/// Test whether a point lies inside a polygon with optional interior rings (holes).
///
/// Returns `true` if the point is inside the exterior ring and outside all interior
/// rings. A point on the boundary of the exterior ring is considered inside; a point
/// on the boundary of an interior ring is considered outside (excluded by the hole).
pub fn point_in_polygon(
    px: f64,
    py: f64,
    exterior: &[(f64, f64)],
    interiors: &[&[(f64, f64)]],
) -> bool {
    match point_in_ring(px, py, exterior) {
        PipResult::Outside => return false,
        PipResult::OnBoundary => return true,
        PipResult::Inside => {}
    }
    for hole in interiors {
        match point_in_ring(px, py, hole) {
            PipResult::Outside => {}
            PipResult::OnBoundary | PipResult::Inside => return false,
        }
    }
    true
}

/// Serialize polygon ring vertices as compact binary (little-endian f64 pairs).
///
/// Each vertex is stored as `[lat_bytes, lon_bytes]` — 16 bytes per point.
/// The ring is stored without repeating the closing vertex.
pub(crate) fn serialize_vertices(ring: &[(f64, f64)]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(ring.len() * 16);
    for &(lat, lon) in ring {
        buf.extend_from_slice(&lat.to_le_bytes());
        buf.extend_from_slice(&lon.to_le_bytes());
    }
    buf
}

/// Deserialize polygon ring vertices from compact binary (little-endian f64 pairs).
///
/// Each vertex is 16 bytes: `[lat, lon]`. The number of vertices is `blob.len() / 16`.
pub(crate) fn deserialize_vertices(blob: &[u8]) -> Result<Vec<(f64, f64)>, String> {
    if !blob.len().is_multiple_of(16) {
        return Err(format!(
            "deserialize_vertices: blob length {} is not a multiple of 16",
            blob.len()
        ));
    }
    let n = blob.len() / 16;
    let mut ring = Vec::with_capacity(n);
    for i in 0..n {
        let lat = f64::from_le_bytes(blob[i * 16..i * 16 + 8].try_into().unwrap());
        let lon = f64::from_le_bytes(blob[i * 16 + 8..i * 16 + 16].try_into().unwrap());
        ring.push((lat, lon));
    }
    Ok(ring)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let restored = deserialize_vertices(&blob).unwrap();
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
        let ring = deserialize_vertices(&blob).unwrap();
        assert!(ring.is_empty(), "empty blob should yield empty vec");
    }

    #[test]
    fn test_deserialize_vertices_trailing() {
        let ring = vec![(-6.16, 106.85)];
        let mut blob = serialize_vertices(&ring);
        blob.push(0xFF);
        assert!(
            deserialize_vertices(&blob).is_err(),
            "trailing byte should error"
        );
    }

    #[test]
    fn test_deserialize_vertices_aligned() {
        let ring = vec![(-6.16, 106.85), (-6.20, 106.90)];
        let blob = serialize_vertices(&ring);
        assert!(blob.len().is_multiple_of(16));
        let restored = deserialize_vertices(&blob).unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0], (-6.16, 106.85));
        assert_eq!(restored[1], (-6.20, 106.90));
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
}
