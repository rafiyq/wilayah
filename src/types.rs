use core::fmt;
use serde::Serialize;

/// Earth's mean radius in kilometers, used by [`haversine_km`].
pub const EARTH_RADIUS_KM: f64 = 6371.0;

/// Maximum number of results returned by [`Database::find_nearest`](crate::Database::find_nearest).
pub const NEAREST_MAX_LIMIT: usize = 20;

/// Maximum number of results returned by [`Database::find_by_name`](crate::Database::find_by_name).
pub const SEARCH_MAX_LIMIT: usize = 100;

/// Maximum number of results per page returned by [`Database::find_by_code_prefix`](crate::Database::find_by_code_prefix).
pub const CODE_PREFIX_MAX_LIMIT: usize = 1000;

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

/// Metadata about the embedded location database.
///
/// Returned by [`data_info()`](crate::data_info) and
/// [`Database::data_info()`](crate::Database::data_info). Contains information
/// about the data source, the government decree it's based on, the number of
/// villages, and when the database was built.
///
/// Metadata is read from the `db_meta` table embedded in the database itself,
/// so it is always correct regardless of how the binary was built (pipeline mode
/// or download mode).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DataInfo {
    /// The upstream data source (e.g., `"official"` or `"release"`).
    pub source: String,
    /// The government decree this data is based on
    /// (e.g., `"Kepmendagri No 300.2.2-2138 Tahun 2025"`).
    pub decree: String,
    /// The number of villages in the database.
    pub village_count: u32,
    /// Unix timestamp (seconds since epoch) of when this database was built.
    pub build_date: u64,
}

/// A village record with administrative hierarchy and coordinates.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Village {
    /// BMKG-compatible administrative code (e.g., `31.71.03.1001`)
    pub code: String,
    /// Village (desa/kelurahan) name
    pub name: String,
    /// District (kecamatan) name
    pub district: String,
    /// City/regency (kabupaten/kota) name
    pub city: String,
    /// Province name
    pub province: String,
    /// Latitude coordinate
    pub lat: f64,
    /// Longitude coordinate
    pub lon: f64,
    /// Distance from query point in kilometers.
    /// Only set by `find_nearest()`, always `None` from `find_by_name()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist_km: Option<f64>,
}

impl fmt::Display for Village {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} — {}, {}, {} ({})",
            self.name, self.district, self.city, self.province, self.code
        )
    }
}

/// Method used to determine the administrative location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LocateMethod {
    /// Matched by nearest village centroid (Haversine distance).
    Nearest,
    /// Matched by polygon containment — the query point falls within the
    /// village's administrative boundary polygon.
    Contained,
}

impl fmt::Display for LocateMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LocateMethod::Nearest => write!(f, "nearest"),
            LocateMethod::Contained => write!(f, "contained"),
        }
    }
}

/// A single level of the administrative hierarchy with code and name.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AdminLevel {
    /// Administrative code for this level (e.g., `"31"`, `"31.71"`, `"31.71.03"`).
    pub code: String,
    /// Name of this administrative unit.
    pub name: String,
}

impl fmt::Display for AdminLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.code, self.name)
    }
}

/// Result of a reverse-geocode lookup showing the full administrative hierarchy.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Location {
    /// Province level (code + name).
    pub province: AdminLevel,
    /// City/regency (kabupaten/kota) level.
    pub city: AdminLevel,
    /// District (kecamatan) level.
    pub district: AdminLevel,
    /// Village (desa/kelurahan) name.
    pub village: String,
    /// Village administrative code (e.g., `"31.71.03.1001"`).
    pub village_code: String,
    /// Latitude of the matched village centroid.
    pub lat: f64,
    /// Longitude of the matched village centroid.
    pub lon: f64,
    /// Distance in km from the query point to the matched village centroid.
    pub dist_km: f64,
    /// Method used to determine this location.
    pub method: LocateMethod,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.province)?;
        writeln!(f, "  {}", self.city)?;
        writeln!(f, "  {}", self.district)?;
        writeln!(
            f,
            "  {} {} ({:.1} km, {})",
            self.village_code, self.village, self.dist_km, self.method
        )
    }
}

/// Build a [`Location`] from a village record by parsing its administrative code.
///
/// Splits the `code` field (e.g., `"31.71.03.1001"`) into province, city, and
/// district codes, and combines them with the village's administrative names.
/// Returns `None` if the code doesn't have exactly 4 dot-separated parts.
pub fn location_from_village(v: &Village, dist_km: f64, method: LocateMethod) -> Option<Location> {
    let parts: Vec<&str> = v.code.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    Some(Location {
        province: AdminLevel {
            code: parts[0].to_string(),
            name: v.province.clone(),
        },
        city: AdminLevel {
            code: format!("{}.{}", parts[0], parts[1]),
            name: v.city.clone(),
        },
        district: AdminLevel {
            code: format!("{}.{}.{}", parts[0], parts[1], parts[2]),
            name: v.district.clone(),
        },
        village: v.name.clone(),
        village_code: v.code.clone(),
        lat: v.lat,
        lon: v.lon,
        dist_km,
        method,
    })
}

/// Result of an unambiguous name lookup.
///
/// Implements [`fmt::Display`] for friendly CLI output:
///
/// ```ignore
/// match result {
///     LookupResult::Found(v) => println!("{v}"),
///     LookupResult::Ambiguous(list) => println!("{result}"),
///     LookupResult::NotFound => eprintln!("{result}"),
/// }
/// ```
#[derive(Debug, Clone, Serialize)]
pub enum LookupResult {
    /// Exactly one match
    Found(Village),
    /// Multiple matches found
    Ambiguous(Vec<Village>),
    /// No matches
    NotFound,
}

impl fmt::Display for LookupResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LookupResult::Found(v) => write!(f, "{}", v),
            LookupResult::Ambiguous(list) => {
                writeln!(f, "Found {} matching villages:", list.len())?;
                for (i, v) in list.iter().enumerate() {
                    writeln!(f, "  {}. {}", i + 1, v)?;
                }
                write!(
                    f,
                    "Use a more specific query (e.g., include city or province)"
                )
            }
            LookupResult::NotFound => write!(f, "No matching village found"),
        }
    }
}

/// Paginated result from a code prefix lookup.
#[derive(Debug, Clone, Serialize)]
pub struct PrefixResult {
    /// The villages in this page of results.
    pub villages: Vec<Village>,
    /// Total number of villages matching the prefix.
    pub total: usize,
    /// Whether more results exist beyond this page.
    pub has_more: bool,
}

impl fmt::Display for PrefixResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} result(s), total: {}, has_more: {}",
            self.villages.len(),
            self.total,
            self.has_more,
        )
    }
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

const PIP_EPSILON: f64 = 1e-10;

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
    let n = blob.len() / 16;
    let mut ring = Vec::with_capacity(n);
    for i in 0..n {
        let lat = f64::from_le_bytes(blob[i * 16..i * 16 + 8].try_into().unwrap());
        let lon = f64::from_le_bytes(blob[i * 16 + 8..i * 16 + 16].try_into().unwrap());
        ring.push((lat, lon));
    }
    ring
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
    fn test_pip_horizontal_edge_no_false_crossing() {
        let square = [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
        assert_eq!(point_in_ring(0.5, 0.5, &square), PipResult::Inside);
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
}
