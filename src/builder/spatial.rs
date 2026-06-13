//! Builder-specific geometry helpers for classifying polygon rings.

use crate::geometry;

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
