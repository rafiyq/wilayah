use serde::Serialize;
use worker::*;

mod haversine {
    pub const EARTH_RADIUS_KM: f64 = 6371.0;

    pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        let dlat = (lat2 - lat1).to_radians();
        let dlon = (lon2 - lon1).to_radians();
        let a = (dlat / 2.0).sin().powi(2)
            + lat1.to_radians().cos()
                * lat2.to_radians().cos()
                * (dlon / 2.0).sin().powi(2);
        EARTH_RADIUS_KM * 2.0 * a.sqrt().asin()
    }
}

mod types {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct VillageRow {
        pub kode: String,
        pub nama: String,
        pub kecamatan: String,
        pub kota: String,
        pub provinsi: String,
        pub lat: f64,
        pub lon: f64,
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct Village {
        #[serde(flatten)]
        pub row: VillageRow,
        pub dist_km: f64,
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct AdminLevel {
        pub code: String,
        pub name: String,
    }

    #[derive(Debug, Clone, Serialize)]
    #[allow(dead_code)]
    pub enum LocateMethod {
        Nearest,
        Contained,
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct Location {
        pub province: AdminLevel,
        pub city: AdminLevel,
        pub district: AdminLevel,
        pub village: String,
        pub village_code: String,
        pub lat: f64,
        pub lon: f64,
        pub dist_km: f64,
        pub method: LocateMethod,
    }
}

use haversine::haversine_km;
use types::*;

#[derive(Serialize)]
struct IndexResponse {
    name: String,
    version: String,
    village_count: i64,
}

#[derive(Serialize)]
struct NearestResponse {
    results: Vec<Village>,
}

#[derive(Serialize)]
struct SearchResponse {
    results: Vec<VillageRow>,
}

#[derive(Serialize)]
struct CodeResponse {
    result: Option<VillageRow>,
}

#[derive(Serialize)]
struct CodePrefixResponse {
    results: Vec<VillageRow>,
    total: i64,
    has_more: bool,
}

#[derive(Serialize)]
struct LocateResponse {
    result: Option<Location>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn cors_headers() -> Headers {
    let headers = Headers::new();
    headers
}

fn with_cors(response: Result<Response>) -> Result<Response> {
    response.map(|r| {
        let h = cors_headers();
        h.set("Access-Control-Allow-Origin", "*").unwrap();
        h.set("Access-Control-Allow-Methods", "GET").unwrap();
        h.set("Access-Control-Allow-Headers", "*").unwrap();
        r.with_headers(h)
    })
}

fn error_response(msg: &str, status: u16) -> Result<Response> {
    let body = serde_json::to_string(&ErrorResponse {
        error: msg.to_string(),
    })
    .map_err(|e| Error::from(format!("serialize error: {e}")))?;
    with_cors(Response::error(&body, status))
}

fn query_param(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

fn parse_f64_param(url: &Url, key: &str) -> Option<f64> {
    query_param(url, key).and_then(|v| v.parse().ok())
}

fn parse_usize_param(url: &Url, key: &str, default: usize) -> usize {
    query_param(url, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn location_from_village(v: &VillageRow, dist_km: f64) -> Option<Location> {
    let parts: Vec<&str> = v.kode.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    Some(Location {
        province: AdminLevel {
            code: parts[0].to_string(),
            name: v.provinsi.clone(),
        },
        city: AdminLevel {
            code: format!("{}.{}", parts[0], parts[1]),
            name: v.kota.clone(),
        },
        district: AdminLevel {
            code: format!("{}.{}.{}", parts[0], parts[1], parts[2]),
            name: v.kecamatan.clone(),
        },
        village: v.nama.clone(),
        village_code: v.kode.clone(),
        lat: v.lat,
        lon: v.lon,
        dist_km,
        method: LocateMethod::Nearest,
    })
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let router = Router::new();

    router
        .get_async("/", |_req, ctx| async move {
            let d1 = ctx.env.d1("DB")?;
            let count: i64 = d1
                .prepare("SELECT COUNT(*) as cnt FROM locations")
                .first::<i64>(Some("cnt"))
                .await?
                .unwrap_or(0);
            with_cors(Response::from_json(&IndexResponse {
                name: "wilayah".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                village_count: count,
            }))
        })
        .get_async("/nearest", |req, ctx| async move {
            let url = req.url()?;
            let lat = match parse_f64_param(&url, "lat") {
                Some(v) if (-90.0..=90.0).contains(&v) => v,
                _ => return error_response("Invalid or missing 'lat' parameter", 400),
            };
            let lon = match parse_f64_param(&url, "lon") {
                Some(v) if (-180.0..=180.0).contains(&v) => v,
                _ => return error_response("Invalid or missing 'lon' parameter", 400),
            };
            let limit = parse_usize_param(&url, "limit", 5).clamp(1, 20);

            let d1 = ctx.env.d1("DB")?;

            let deltas: [f64; 10] =
                [0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 15.0, 45.0, 180.0];
            for &delta in &deltas {
                let sql = "SELECT kode, nama, kecamatan, kota, provinsi, lat, lon \
                           FROM locations \
                           WHERE lat BETWEEN ?1 AND ?2 AND lon BETWEEN ?3 AND ?4 \
                           LIMIT 200";
                let stmt = d1.prepare(sql);
                let query = stmt.bind(&[
                    (lat - delta).into(),
                    (lat + delta).into(),
                    (lon - delta).into(),
                    (lon + delta).into(),
                ])?;

                let rows: Vec<VillageRow> = query.all().await?.results()?;
                if rows.is_empty() {
                    continue;
                }

                let mut candidates: Vec<Village> = rows
                    .iter()
                    .map(|r| Village {
                        row: r.clone(),
                        dist_km: haversine_km(lat, lon, r.lat, r.lon),
                    })
                    .collect();
                candidates.sort_by(|a, b| a.dist_km.partial_cmp(&b.dist_km).unwrap());
                candidates.truncate(limit);
                return with_cors(Response::from_json(&NearestResponse {
                    results: candidates,
                }));
            }
            with_cors(Response::from_json(&NearestResponse { results: vec![] }))
        })
        .get_async("/search", |req, ctx| async move {
            let url = req.url()?;
            let q = match query_param(&url, "q") {
                Some(v) if !v.is_empty() => v,
                _ => return error_response("Query parameter 'q' is required", 400),
            };
            let limit = parse_usize_param(&url, "limit", 10).clamp(1, 100);
            let pattern = format!("%{q}%");

            let d1 = ctx.env.d1("DB")?;
            let sql = "SELECT kode, nama, kecamatan, kota, provinsi, lat, lon \
                       FROM locations \
                       WHERE nama LIKE ?1 OR kecamatan LIKE ?1 \
                       OR kota LIKE ?1 OR provinsi LIKE ?1 \
                       LIMIT ?2";
            let stmt = d1.prepare(sql);
            let query = stmt.bind(&[pattern.into(), (limit as f64).into()])?;
            let rows: Vec<VillageRow> = query.all().await?.results()?;
            with_cors(Response::from_json(&SearchResponse { results: rows }))
        })
        .get_async("/code", |req, ctx| async move {
            let url = req.url()?;
            let d1 = ctx.env.d1("DB")?;

            if let Some(q) = query_param(&url, "q") {
                let stmt = d1.prepare(
                    "SELECT kode, nama, kecamatan, kota, provinsi, lat, lon \
                     FROM locations WHERE kode = ?1",
                );
                let query = stmt.bind(&[q.into()])?;
                let result: Option<VillageRow> = query.first(None).await?;
                return with_cors(Response::from_json(&CodeResponse { result }));
            }

            if let Some(prefix) = query_param(&url, "prefix") {
                let limit = parse_usize_param(&url, "limit", 100).clamp(1, 1000);
                let offset = parse_usize_param(&url, "offset", 0);
                let pattern = format!("{prefix}%");

                let count_sql =
                    "SELECT COUNT(*) as cnt FROM locations WHERE kode LIKE ?1";
                let count_stmt = d1.prepare(count_sql);
                let count_query = count_stmt.bind(&[pattern.clone().into()])?;
                let total: i64 = count_query
                    .first::<i64>(Some("cnt"))
                    .await?
                    .unwrap_or(0);

                let sql = "SELECT kode, nama, kecamatan, kota, provinsi, lat, lon \
                           FROM locations WHERE kode LIKE ?1 \
                           ORDER BY kode LIMIT ?2 OFFSET ?3";
                let stmt = d1.prepare(sql);
                let query = stmt.bind(&[
                    pattern.into(),
                    (limit as f64).into(),
                    (offset as f64).into(),
                ])?;
                let rows: Vec<VillageRow> = query.all().await?.results()?;
                let has_more = (offset + rows.len()) < total as usize;
                return with_cors(Response::from_json(&CodePrefixResponse {
                    results: rows,
                    total,
                    has_more,
                }));
            }

            error_response(
                "Provide either 'q' (exact code) or 'prefix' (code prefix)",
                400,
            )
        })
        .get_async("/locate", |req, ctx| async move {
            let url = req.url()?;
            let lat = match parse_f64_param(&url, "lat") {
                Some(v) if (-90.0..=90.0).contains(&v) => v,
                _ => return error_response("Invalid or missing 'lat' parameter", 400),
            };
            let lon = match parse_f64_param(&url, "lon") {
                Some(v) if (-180.0..=180.0).contains(&v) => v,
                _ => return error_response("Invalid or missing 'lon' parameter", 400),
            };

            let d1 = ctx.env.d1("DB")?;

            let deltas: [f64; 10] =
                [0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 15.0, 45.0, 180.0];
            for &delta in &deltas {
                let sql = "SELECT kode, nama, kecamatan, kota, provinsi, lat, lon \
                           FROM locations \
                           WHERE lat BETWEEN ?1 AND ?2 AND lon BETWEEN ?3 AND ?4 \
                           LIMIT 200";
                let stmt = d1.prepare(sql);
                let query = stmt.bind(&[
                    (lat - delta).into(),
                    (lat + delta).into(),
                    (lon - delta).into(),
                    (lon + delta).into(),
                ])?;

                let rows: Vec<VillageRow> = query.all().await?.results()?;
                if rows.is_empty() {
                    continue;
                }

                let mut candidates: Vec<(VillageRow, f64)> = rows
                    .into_iter()
                    .map(|r| {
                        let d = haversine_km(lat, lon, r.lat, r.lon);
                        (r, d)
                    })
                    .collect();
                candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

                if let Some((village, dist_km)) = candidates.into_iter().next() {
                    if let Some(loc) = location_from_village(&village, dist_km) {
                        return with_cors(Response::from_json(&LocateResponse {
                            result: Some(loc),
                        }));
                    }
                }
            }
            with_cors(Response::from_json(&LocateResponse { result: None }))
        })
        .run(req, env)
        .await
}
