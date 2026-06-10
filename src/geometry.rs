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
///
/// Returns `(min_lat, max_lat, min_lon, max_lon)`.
pub fn bbox(ring: &[(f64, f64)]) -> (f64, f64, f64, f64) {
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
pub fn serialize_vertices(ring: &[(f64, f64)]) -> Vec<u8> {
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
pub fn deserialize_vertices(blob: &[u8]) -> Vec<(f64, f64)> {
    debug_assert!(
        blob.len().is_multiple_of(16),
        "deserialize_vertices: blob length {} is not a multiple of 16",
        blob.len()
    );
    let n = blob.len() / 16;
    let mut ring = Vec::with_capacity(n);
    for i in 0..n {
        let lat = f64::from_le_bytes(blob[i * 16..i * 16 + 8].try_into().unwrap());
        let lon = f64::from_le_bytes(blob[i * 16 + 8..i * 16 + 16].try_into().unwrap());
        ring.push((lat, lon));
    }
    ring
}
