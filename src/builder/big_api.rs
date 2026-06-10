//! BIG ArcGIS API fetching and village data extraction.

use super::spatial;
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
                spatial::extract_rings(&r)
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
                    let (centroid_lat, centroid_lon) = spatial::compute_centroid(geom);
                    let extracted_rings = if include_polygons {
                        spatial::extract_rings(geom)
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
