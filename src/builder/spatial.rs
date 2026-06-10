//! Builder-specific geometry helpers for processing BIG ArcGIS polygon data.

use crate::geometry;

/// Compute the centroid of a polygon feature from ArcGIS JSON geometry.
///
/// Handles both `rings` format (ArcGIS) and `coordinates` format (GeoJSON).
/// For multi-ring features, uses the ring with the most vertices.
/// Returns `(lat, lon)`.
pub(crate) fn compute_centroid(geometry: &serde_json::Value) -> (f64, f64) {
    let mut rings: Vec<&[serde_json::Value]> = Vec::new();

    if let Some(rings_array) = geometry.get("rings").and_then(|r| r.as_array()) {
        for ring_val in rings_array {
            if let Some(ring) = ring_val.as_array() {
                rings.push(ring);
            }
        }
    } else if let Some(coord_arrays) = geometry.get("coordinates").and_then(|c| c.as_array()) {
        if let Some(first) = coord_arrays.first() {
            if first.get(0).map(|r| r.is_array()).unwrap_or(false) {
                for poly in coord_arrays {
                    if let Some(poly_rings) = poly.as_array() {
                        if let Some(outer) = poly_rings.first() {
                            if let Some(outer_ring) = outer.as_array() {
                                rings.push(outer_ring);
                            }
                        }
                    }
                }
            } else {
                rings.push(coord_arrays);
            }
        }
    }

    if rings.is_empty() {
        return (0.0, 0.0);
    }

    let mut largest_ring = &rings[0];
    let mut max_len = 0;
    for ring in &rings {
        if ring.len() > max_len {
            max_len = ring.len();
            largest_ring = ring;
        }
    }

    polygon_centroid(largest_ring)
}

/// Extract polygon rings from ArcGIS JSON geometry.
///
/// Returns `Some(Vec<ring>)` where each ring is `Vec<[lat, lon]>`.
/// Returns `None` if no valid rings are found.
pub(crate) fn extract_rings(geometry: &serde_json::Value) -> Option<Vec<Vec<[f64; 2]>>> {
    if let Some(rings_array) = geometry.get("rings").and_then(|r| r.as_array()) {
        let result: Vec<Vec<[f64; 2]>> = rings_array
            .iter()
            .filter_map(|ring_val| {
                ring_val.as_array().map(|ring| {
                    ring.iter()
                        .filter_map(|pt| {
                            let lon = pt.get(0)?.as_f64()?;
                            let lat = pt.get(1)?.as_f64()?;
                            Some([lat, lon])
                        })
                        .collect::<Vec<[f64; 2]>>()
                })
            })
            .collect();
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    } else {
        None
    }
}

/// Compute the centroid of a single polygon ring using the shoelace formula.
///
/// Input ring uses `[lon, lat]` order (ArcGIS convention).
/// Returns `(lat, lon)`.
pub(crate) fn polygon_centroid(ring: &[serde_json::Value]) -> (f64, f64) {
    if ring.len() < 3 {
        return (0.0, 0.0);
    }

    let mut area = 0.0_f64;
    let mut cx = 0.0_f64;
    let mut cy = 0.0_f64;
    let n = ring.len();

    for i in 0..n {
        let j = (i + 1) % n;

        let x_i = ring[i].get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y_i = ring[i].get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let x_j = ring[j].get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y_j = ring[j].get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);

        let cross = x_i * y_j - x_j * y_i;
        area += cross;
        cx += (x_i + x_j) * cross;
        cy += (y_i + y_j) * cross;
    }

    area *= 0.5;
    if area.abs() < 1e-10 {
        let mut sx = 0.0_f64;
        let mut sy = 0.0_f64;
        for pt in ring {
            sx += pt.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
            sy += pt.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
        }
        return (sy / ring.len() as f64, sx / ring.len() as f64);
    }

    cx /= 6.0 * area;
    cy /= 6.0 * area;

    (cy, cx)
}

/// Classify rings as exterior or interior (holes) using spatial containment.
///
/// For each ring, if it is fully contained within a larger ring, it is
/// classified as `"interior"` (hole). Otherwise it is `"exterior"`.
pub(crate) fn classify_rings(rings: &[Vec<[f64; 2]>]) -> Vec<&'static str> {
    if rings.len() <= 1 {
        return vec!["exterior"; rings.len()];
    }

    let bboxes: Vec<(f64, f64, f64, f64)> = rings
        .iter()
        .map(|ring| {
            let vertices: Vec<(f64, f64)> = ring.iter().map(|&[lat, lon]| (lat, lon)).collect();
            geometry::bbox(&vertices)
        })
        .collect();

    let areas: Vec<f64> = rings
        .iter()
        .map(|ring| {
            let n = ring.len();
            if n < 3 {
                return 0.0_f64;
            }
            let mut area = 0.0_f64;
            for i in 0..n {
                let j = (i + 1) % n;
                let (lat_i, lon_i) = (ring[i][0], ring[i][1]);
                let (lat_j, lon_j) = (ring[j][0], ring[j][1]);
                area += lon_i * lat_j - lon_j * lat_i;
            }
            area.abs() * 0.5
        })
        .collect();

    let mut types = vec!["exterior"; rings.len()];

    for i in 0..rings.len() {
        if areas[i] < 1e-10 {
            continue;
        }
        let (min_lat_i, max_lat_i, min_lon_i, max_lon_i) = bboxes[i];
        for j in 0..rings.len() {
            if i == j || areas[j] <= areas[i] {
                continue;
            }
            let (min_lat_j, max_lat_j, min_lon_j, max_lon_j) = bboxes[j];
            if min_lat_j <= min_lat_i
                && max_lat_j >= max_lat_i
                && min_lon_j <= min_lon_i
                && max_lon_j >= max_lon_i
            {
                let test_pt = (rings[i][0][0], rings[i][0][1]);
                let exterior: Vec<(f64, f64)> =
                    rings[j].iter().map(|&[lat, lon]| (lat, lon)).collect();
                if geometry::point_in_polygon(test_pt.0, test_pt.1, &exterior, &[]) {
                    types[i] = "interior";
                    break;
                }
            }
        }
    }

    types
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_polygon_centroid_square() {
        let ring = vec![
            json!([0.0, 0.0]),
            json!([2.0, 0.0]),
            json!([2.0, 2.0]),
            json!([0.0, 2.0]),
        ];
        let (lat, lon) = polygon_centroid(&ring);
        assert!((lat - 1.0).abs() < 1e-10);
        assert!((lon - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_polygon_centroid_too_few_points() {
        let ring = vec![json!([0.0, 0.0]), json!([1.0, 1.0])];
        let (lat, lon) = polygon_centroid(&ring);
        assert_eq!(lat, 0.0);
        assert_eq!(lon, 0.0);
    }

    #[test]
    fn test_polygon_centroid_collinear_fallback() {
        let ring = vec![json!([0.0, 0.0]), json!([2.0, 2.0]), json!([4.0, 4.0])];
        let (lat, lon) = polygon_centroid(&ring);
        assert!((lat - 2.0).abs() < 1e-10);
        assert!((lon - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_compute_centroid_rings_format() {
        let geom = json!({
            "rings": [
                [
                    [100.0, -4.0],
                    [100.0, -6.0],
                    [102.0, -6.0],
                    [102.0, -4.0],
                    [100.0, -4.0]
                ]
            ]
        });
        let (lat, lon) = compute_centroid(&geom);
        assert!((lat - -5.0).abs() < 0.1);
        assert!((lon - 101.0).abs() < 0.1);
    }

    #[test]
    fn test_compute_centroid_coordinates_format() {
        let geom = json!({
            "coordinates": [
                [
                    [
                        [100.0, 0.0],
                        [101.0, 0.0],
                        [101.0, 1.0],
                        [100.0, 1.0],
                        [100.0, 0.0]
                    ]
                ]
            ]
        });
        let (lat, lon) = compute_centroid(&geom);
        assert!((lat - 0.5).abs() < 0.1);
        assert!((lon - 100.5).abs() < 0.1);
    }

    #[test]
    fn test_compute_centroid_empty() {
        let geom = json!({});
        let (lat, lon) = compute_centroid(&geom);
        assert_eq!(lat, 0.0);
        assert_eq!(lon, 0.0);
    }

    #[test]
    fn test_extract_rings_rings_format() {
        let geom = json!({
            "rings": [
                [[0.0, -6.0], [0.0, -8.0], [2.0, -8.0], [2.0, -6.0], [0.0, -6.0]],
                [[0.5, -6.5], [0.5, -7.5], [1.5, -7.5], [1.5, -6.5], [0.5, -6.5]]
            ]
        });
        let rings = extract_rings(&geom).expect("should extract rings");
        assert_eq!(rings.len(), 2);
        assert_eq!(rings[0].len(), 5);
        assert_eq!(rings[0][0], [-6.0, 0.0]);
    }

    #[test]
    fn test_extract_rings_no_rings_key() {
        let geom = json!({"coordinates": []});
        assert!(extract_rings(&geom).is_none());
    }

    #[test]
    fn test_classify_rings_separate() {
        let ring1 = vec![
            [0.0, 0.0],
            [0.0, 10.0],
            [10.0, 10.0],
            [10.0, 0.0],
            [0.0, 0.0],
        ];
        let ring2 = vec![
            [20.0, 20.0],
            [20.0, 30.0],
            [30.0, 30.0],
            [30.0, 20.0],
            [20.0, 20.0],
        ];
        let types = classify_rings(&[ring1, ring2]);
        assert_eq!(types, vec!["exterior", "exterior"]);
    }

    #[test]
    fn test_classify_rings_with_hole() {
        let exterior = vec![
            [0.0, 0.0],
            [0.0, 10.0],
            [10.0, 10.0],
            [10.0, 0.0],
            [0.0, 0.0],
        ];
        let hole = vec![[2.0, 2.0], [2.0, 8.0], [8.0, 8.0], [8.0, 2.0], [2.0, 2.0]];
        let types = classify_rings(&[exterior, hole]);
        assert_eq!(types[0], "exterior");
        assert_eq!(types[1], "interior");
    }
}
