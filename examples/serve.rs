use axum::{
    extract::{Query, State},
    http::{Method, StatusCode},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::EnvFilter;
use wilayah::{Database, Village};

fn validate_coords(lat: f64, lon: f64) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if lat < -90.0 || lat > 90.0 || lon < -180.0 || lon > 180.0 {
        Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid coordinates".into(),
            }),
        ))
    } else {
        Ok(())
    }
}

fn internal_error(e: wilayah::Error) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: format!("{e}"),
        }),
    )
}

fn bad_request(msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse { error: msg.into() }),
    )
}

struct AppState {
    db: Database,
}

impl AppState {
    fn new(poly_path: Option<&str>) -> Result<Self, Box<dyn std::error::Error>> {
        let db = if let Some(path) = poly_path {
            Database::open_with_polygons(path)?
        } else {
            Database::open()?
        };
        let count = db.village_count()?;
        let poly_status = if db.has_polygons() {
            "enabled"
        } else {
            "disabled"
        };
        info!("Database loaded: {count} villages, polygon containment: {poly_status}");
        Ok(Self { db })
    }
}

#[derive(Debug, Deserialize)]
struct NearestParams {
    lat: f64,
    lon: f64,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
struct SearchParams {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
struct LocateParams {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Deserialize)]
struct CodeParams {
    q: Option<String>,
    prefix: Option<String>,
    #[serde(default = "default_code_prefix_limit")]
    limit: usize,
    #[serde(default = "default_offset")]
    offset: usize,
}

fn default_limit() -> usize {
    5
}

fn default_code_prefix_limit() -> usize {
    100
}

fn default_offset() -> usize {
    0
}

#[derive(Serialize)]
struct IndexResponse {
    name: String,
    version: String,
    village_count: u32,
}

#[derive(Serialize)]
struct SearchResponse {
    results: Vec<Village>,
}

#[derive(Serialize)]
struct CodeResponse {
    result: Option<Village>,
}

#[derive(Serialize)]
struct CodePrefixResponse {
    results: Vec<Village>,
    total: usize,
    has_more: bool,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct LocateResponse {
    result: Option<wilayah::Location>,
}

async fn index(state: State<Arc<AppState>>) -> Json<IndexResponse> {
    let count = state.db.village_count().unwrap_or(0);
    Json(IndexResponse {
        name: "wilayah".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        village_count: count,
    })
}

async fn nearest(
    state: State<Arc<AppState>>,
    Query(params): Query<NearestParams>,
) -> Result<Json<Vec<Village>>, (StatusCode, Json<ErrorResponse>)> {
    validate_coords(params.lat, params.lon)?;
    info!(
        "nearest: lat={}, lon={}, limit={}",
        params.lat, params.lon, params.limit
    );
    let results = state
        .db
        .find_nearest(params.lat, params.lon, params.limit)
        .map_err(internal_error)?;
    Ok(Json(results))
}

async fn search(
    state: State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, (StatusCode, Json<ErrorResponse>)> {
    if params.q.trim().is_empty() {
        return Err(bad_request("Query parameter 'q' is required"));
    }
    info!("search: q={}, limit={}", params.q, params.limit);
    let results = state
        .db
        .find_by_name(&params.q, params.limit)
        .map_err(internal_error)?;
    Ok(Json(SearchResponse { results }))
}

async fn code(
    state: State<Arc<AppState>>,
    Query(params): Query<CodeParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    if let Some(q) = &params.q {
        let code = q.trim();
        if code.is_empty() {
            return Err(bad_request("Parameter 'q' must not be empty"));
        }
        info!("code: exact lookup for {}", code);
        let result = state.db.find_by_code(code).map_err(internal_error)?;
        return Ok(Json(serde_json::to_value(CodeResponse { result }).unwrap()));
    }
    if let Some(prefix) = &params.prefix {
        let prefix = prefix.trim();
        if prefix.is_empty() {
            return Err(bad_request("Parameter 'prefix' must not be empty"));
        }
        let offset = params.offset;
        info!(
            "code: prefix lookup for {} (limit={}, offset={})",
            prefix, params.limit, offset
        );
        let result = state
            .db
            .find_by_code_prefix(prefix, params.limit, offset)
            .map_err(internal_error)?;
        return Ok(Json(
            serde_json::to_value(CodePrefixResponse {
                results: result.villages,
                total: result.total,
                has_more: result.has_more,
            })
            .unwrap(),
        ));
    }
    Err(bad_request(
        "Provide either 'q' (exact code) or 'prefix' (code prefix)",
    ))
}

async fn locate_handler(
    state: State<Arc<AppState>>,
    Query(params): Query<LocateParams>,
) -> Result<Json<LocateResponse>, (StatusCode, Json<ErrorResponse>)> {
    validate_coords(params.lat, params.lon)?;
    info!("locate: lat={}, lon={}", params.lat, params.lon);
    let result = state
        .db
        .locate(params.lat, params.lon)
        .map_err(internal_error)?;
    Ok(Json(LocateResponse { result }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let args: Vec<String> = std::env::args().collect();
    let poly_path = match args.iter().position(|a| a == "--poly") {
        Some(i) if i + 1 < args.len() => Some(args[i + 1].as_str()),
        _ => None,
    };

    let state = Arc::new(AppState::new(poly_path).expect("failed to initialize database"));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET])
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(index))
        .route("/nearest", get(nearest))
        .route("/search", get(search))
        .route("/code", get(code))
        .route("/locate", get(locate_handler))
        .with_state(state)
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("wilayah API listening on {addr}");
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
