//! BIG ArcGIS API fetching and village data extraction.

use super::util;
use super::PipelineError;
use super::PipelineResultExt;
use std::fs;
use std::path::Path;

const BIG_BATCH_SIZE: usize = 1000;

/// A village record from the BIG ArcGIS API.
pub(crate) struct BigRecord {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) district: String,
    pub(crate) city: String,
    pub(crate) province: String,
    pub(crate) lat: f64,
    pub(crate) lon: f64,
    pub(crate) rings: Option<Vec<Vec<[f64; 2]>>>,
}

fn json_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn json_str_or(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn compute_centroid(geometry: &serde_json::Value) -> (f64, f64) {
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

fn extract_rings(geometry: &serde_json::Value) -> Option<Vec<Vec<[f64; 2]>>> {
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

fn polygon_centroid(ring: &[serde_json::Value]) -> (f64, f64) {
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

/// Fetch BIG village data from ArcGIS API, using a local JSON cache.
pub(crate) fn fetch_big_data(
    api_url: &str,
    cache_dir: &Path,
    force_refresh: bool,
    include_polygons: bool,
) -> Result<Vec<BigRecord>, PipelineError> {
    let cache_filename = if include_polygons {
        "big_villages_with_polygons.json"
    } else {
        "big_villages.json"
    };
    let cache_path = cache_dir.join(cache_filename);

    if !force_refresh && cache_path.exists() {
        return load_big_cache(&cache_path, include_polygons);
    }

    let records = fetch_big_from_api(api_url, cache_dir, include_polygons)?;
    save_big_cache(&cache_path, &records, include_polygons)?;
    Ok(records)
}

fn load_big_cache(
    cache_path: &Path,
    include_polygons: bool,
) -> Result<Vec<BigRecord>, PipelineError> {
    let content = fs::read_to_string(cache_path).ctx("failed to read BIG cache")?;
    let records: Vec<serde_json::Value> =
        serde_json::from_str(&content).ctx("failed to parse BIG cache")?;
    let mut result = Vec::with_capacity(records.len());
    for r in records {
        if let (Some(code), Some(lat), Some(lon)) = (
            r.get("code").and_then(|v| v.as_str()),
            r.get("lat").and_then(|v| v.as_f64()),
            r.get("lon").and_then(|v| v.as_f64()),
        ) {
            let rings = if include_polygons {
                extract_rings(&r)
            } else {
                None
            };
            result.push(BigRecord {
                code: code.to_string(),
                name: json_str_or(&r, "name"),
                district: json_str_or(&r, "district"),
                city: json_str_or(&r, "city"),
                province: json_str_or(&r, "province"),
                lat,
                lon,
                rings,
            });
        }
    }
    eprintln!("Loaded {} BIG village records from cache", result.len());
    Ok(result)
}

fn fetch_big_from_api(
    api_url: &str,
    cache_dir: &Path,
    include_polygons: bool,
) -> Result<Vec<BigRecord>, PipelineError> {
    eprintln!("Fetching BIG village data from ArcGIS API...");
    fs::create_dir_all(cache_dir).ctx("failed to create cache directory")?;

    let mut all_records = Vec::new();
    let mut offset = 0;
    let mut batch_num = 0;

    loop {
        batch_num += 1;
        let url = format!(
            "{}?where=KDEPUM+IS+NOT+NULL\
            &outFields=KDEPUM,WADMKD,WADMKC,WADMKK,WADMPR\
            &returnGeometry=true\
            &f=json\
            &resultRecordCount={}\u{0026}resultOffset={}",
            api_url, BIG_BATCH_SIZE, offset
        );

        if batch_num % 10 == 1 || batch_num <= 3 {
            eprintln!("Fetching BIG batch {} (offset={})...", batch_num, offset);
        }

        let resp = util::fetch_with_retry(&url, 3)?;
        let json: serde_json::Value =
            serde_json::from_str(&resp).ctx("failed to parse BIG API response")?;

        if let Some(error) = json.get("error") {
            return Err(PipelineError::new(format!("BIG API error: {}", error)));
        }

        let features = json
            .get("features")
            .and_then(|f| f.as_array())
            .ok_or_else(|| PipelineError::new("missing features in BIG response"))?;

        if features.is_empty() {
            break;
        }

        for feature in features {
            let attrs = feature
                .get("attributes")
                .ok_or_else(|| PipelineError::new("missing attributes"))?;
            let code = json_str(attrs, "KDEPUM");
            let name = json_str(attrs, "WADMKD");
            let district = json_str(attrs, "WADMKC");
            let city = json_str(attrs, "WADMKK");
            let province = json_str(attrs, "WADMPR");

            if let (Some(code), Some(name)) = (code, name) {
                let geometry = feature.get("geometry");
                let (lat, lon, rings) = if let Some(geom) = geometry {
                    let (centroid_lat, centroid_lon) = compute_centroid(geom);
                    let extracted_rings = if include_polygons {
                        extract_rings(geom)
                    } else {
                        None
                    };
                    (centroid_lat, centroid_lon, extracted_rings)
                } else {
                    (0.0, 0.0, None)
                };

                all_records.push(BigRecord {
                    code,
                    name,
                    district: district.unwrap_or_default(),
                    city: city.unwrap_or_default(),
                    province: province.unwrap_or_default(),
                    lat,
                    lon,
                    rings,
                });
            }
        }

        if features.len() < BIG_BATCH_SIZE {
            break;
        }

        offset += BIG_BATCH_SIZE;
    }

    eprintln!(
        "Fetched {} BIG village records in {} batches",
        all_records.len(),
        batch_num
    );

    Ok(all_records)
}

fn save_big_cache(
    cache_path: &Path,
    records: &[BigRecord],
    include_polygons: bool,
) -> Result<(), PipelineError> {
    let cache_data: Vec<serde_json::Value> = records
        .iter()
        .map(|r| {
            let mut obj = serde_json::json!({
                "code": r.code,
                "name": r.name,
                "district": r.district,
                "city": r.city,
                "province": r.province,
                "lat": r.lat,
                "lon": r.lon,
            });
            if include_polygons {
                if let Some(rings) = &r.rings {
                    obj["rings"] = serde_json::json!(rings);
                }
            }
            obj
        })
        .collect();
    let cache_json = serde_json::to_string(&cache_data).ctx("failed to serialize BIG cache")?;
    fs::write(cache_path, cache_json).ctx("failed to write BIG cache")?;
    eprintln!("Saved BIG cache to {:?}", cache_path);
    Ok(())
}
