use rusqlite::{functions::FunctionFlags, Connection, Result};
use std::fmt;
use std::sync::OnceLock;

use crate::DataInfo;

const DB_BYTES: &[u8] = include_bytes!(env!("LOCATION_DB_PATH"));

const EARTH_RADIUS_KM: f64 = 6371.0;

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    EARTH_RADIUS_KM * 2.0 * a.sqrt().asin()
}

/// Open the embedded database connection and register the Haversine UDF.
pub fn open_embedded() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA journal_mode = OFF")?;
    conn.deserialize_bytes("main", DB_BYTES)?;

    conn.create_scalar_function(
        "haversine_km",
        4,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        move |ctx| {
            Ok(haversine_km(
                ctx.get::<f64>(0)?,
                ctx.get::<f64>(1)?,
                ctx.get::<f64>(2)?,
                ctx.get::<f64>(3)?,
            ))
        },
    )?;

    Ok(conn)
}

/// Read a key from the `db_meta` table.
fn query_meta(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM db_meta WHERE key = ?1", [key], |row| {
        row.get(0)
    })
    .ok()
}

/// Get metadata about the database from an existing connection.
///
/// Reads the `db_meta` table for decree, source, build date, and village count.
/// Returns default values if the table is missing or keys are absent.
pub fn data_info_from_conn(conn: &Connection) -> DataInfo {
    DataInfo {
        source: query_meta(conn, "source").unwrap_or_else(|| "unknown".to_string()),
        decree: query_meta(conn, "decree").unwrap_or_else(|| "unknown".to_string()),
        village_count: query_meta(conn, "village_count")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        build_date: query_meta(conn, "build_date")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
    }
}

/// Cached `DataInfo` — opened once, reused on subsequent calls.
static CACHED_DATA_INFO: OnceLock<DataInfo> = OnceLock::new();

/// Get metadata about the embedded location database (cached).
///
/// Opens the database on first call and caches the result.
/// Subsequent calls return the cached value without re-opening the database.
pub fn cached_data_info() -> &'static DataInfo {
    CACHED_DATA_INFO.get_or_init(|| {
        let conn = open_embedded().expect("failed to open embedded database for metadata");
        data_info_from_conn(&conn)
    })
}

/// Find nearest villages using RTree spatial index + Haversine distance.
pub fn nearest(conn: &Connection, lat: f64, lon: f64, limit: usize) -> Result<Vec<Village>> {
    let limit = limit.clamp(1, 20);

    let deltas: [f64; 10] = [0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 15.0, 45.0, 180.0];

    for &delta in &deltas {
        let sql = "
            SELECT l.kode, l.nama, l.kecamatan, l.kota, l.provinsi, l.lat, l.lon,
                   haversine_km(?1, ?2, l.lat, l.lon) AS dist
            FROM locations l
            JOIN geo_rtree r ON l.id = r.id
            WHERE r.min_lon <= ?4 AND r.max_lon >= ?3
              AND r.min_lat <= ?6 AND r.max_lat >= ?5
            ORDER BY dist
            LIMIT ?7
        ";

        let mut stmt = conn.prepare_cached(sql)?;
        let rows = stmt.query_map(
            rusqlite::params![
                lat,
                lon,
                lon - delta,
                lon + delta,
                lat - delta,
                lat + delta,
                limit as i64
            ],
            |row| {
                Ok(Village {
                    code: row.get(0)?,
                    name: row.get(1)?,
                    district: row.get(2)?,
                    city: row.get(3)?,
                    province: row.get(4)?,
                    lat: row.get(5)?,
                    lon: row.get(6)?,
                    dist_km: Some(row.get(7)?),
                })
            },
        )?;

        let results: Result<Vec<_>> = rows.collect();
        let results = results?;

        if results.len() >= limit {
            return Ok(results);
        }
    }

    Ok(vec![])
}

/// Search villages by name using FTS5 full-text search.
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<Village>> {
    let limit = limit.clamp(1, 100);

    let sql = "
        SELECT l.kode, l.nama, l.kecamatan, l.kota, l.provinsi, l.lat, l.lon
        FROM locations_fts f
        JOIN locations l ON f.rowid = l.id
        WHERE locations_fts MATCH ?1
        ORDER BY rank
        LIMIT ?2
    ";

    let mut stmt = conn.prepare_cached(sql)?;
    let rows = stmt.query_map(rusqlite::params![query, limit as i64], |row| {
        Ok(Village {
            code: row.get(0)?,
            name: row.get(1)?,
            district: row.get(2)?,
            city: row.get(3)?,
            province: row.get(4)?,
            lat: row.get(5)?,
            lon: row.get(6)?,
            dist_km: None,
        })
    })?;

    rows.collect()
}

/// Lookup a village by its BMKG administrative code (e.g., `31.71.03.1001`).
pub fn by_code(conn: &Connection, code: &str) -> Result<Option<Village>> {
    let mut stmt = conn.prepare_cached(
        "SELECT kode, nama, kecamatan, kota, provinsi, lat, lon
         FROM locations
         WHERE kode = ?1",
    )?;
    let mut rows = stmt.query_map(rusqlite::params![code], |row| {
        Ok(Village {
            code: row.get(0)?,
            name: row.get(1)?,
            district: row.get(2)?,
            city: row.get(3)?,
            province: row.get(4)?,
            lat: row.get(5)?,
            lon: row.get(6)?,
            dist_km: None,
        })
    })?;
    match rows.next() {
        Some(Ok(v)) => Ok(Some(v)),
        Some(Err(e)) => Err(e),
        None => Ok(None),
    }
}

/// Lookup all villages matching an administrative code prefix with pagination.
///
/// Useful for listing all villages in a kecamatan (`"31.71.03"`),
/// kabupaten (`"31.71"`), or province (`"31"`). Returns a paginated
/// result with total count and a `has_more` flag.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `prefix` - Code prefix (e.g., `"31.71.03"`, `"31.71"`, `"31"`)
/// * `limit` - Maximum number of results per page (clamped to 1..1000)
/// * `offset` - Number of results to skip (for pagination)
///
/// Returns a `PrefixResult` containing villages, total count, and pagination flag.
pub fn by_code_prefix(
    conn: &Connection,
    prefix: &str,
    limit: usize,
    offset: usize,
) -> Result<PrefixResult> {
    let limit = limit.clamp(1, 1000);
    let pattern = format!("{}%", prefix);

    // Get total count (COUNT(*) returns i64, cast to usize)
    let total_i64: i64 = conn.query_row(
        "SELECT COUNT(*) FROM locations WHERE kode LIKE ?1",
        [&pattern],
        |row| row.get(0),
    )?;
    let total = total_i64 as usize;

    // Get page of results
    let mut stmt = conn.prepare_cached(
        "SELECT kode, nama, kecamatan, kota, provinsi, lat, lon
         FROM locations
         WHERE kode LIKE ?1
         ORDER BY kode
         LIMIT ?2
         OFFSET ?3",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![pattern, limit as i64, offset as i64],
        |row| {
            Ok(Village {
                code: row.get(0)?,
                name: row.get(1)?,
                district: row.get(2)?,
                city: row.get(3)?,
                province: row.get(4)?,
                lat: row.get(5)?,
                lon: row.get(6)?,
                dist_km: None,
            })
        },
    )?;
    let villages: Vec<Village> = rows.collect::<Result<Vec<_>>>()?;

    let has_more = offset + villages.len() < total;

    Ok(PrefixResult {
        villages,
        total,
        has_more,
    })
}

/// Search for a single unique village by name.
///
/// Returns a single match if exactly one result exists, otherwise returns all
/// matches (up to 20) for the caller to disambiguate.
pub fn search_unique(conn: &Connection, query: &str) -> Result<LookupResult> {
    let mut stmt = conn.prepare_cached(
        "SELECT l.kode, l.nama, l.kecamatan, l.kota, l.provinsi, l.lat, l.lon
         FROM locations_fts f
         JOIN locations l ON f.rowid = l.id
         WHERE locations_fts MATCH ?1
         ORDER BY rank
         LIMIT 20",
    )?;
    let rows = stmt.query_map(rusqlite::params![query], |row| {
        Ok(Village {
            code: row.get(0)?,
            name: row.get(1)?,
            district: row.get(2)?,
            city: row.get(3)?,
            province: row.get(4)?,
            lat: row.get(5)?,
            lon: row.get(6)?,
            dist_km: None,
        })
    })?;
    let results: Vec<_> = rows.collect::<Result<Vec<_>>>()?;

    Ok(match results.len() {
        0 => LookupResult::NotFound,
        1 => LookupResult::Found(results.into_iter().next().unwrap()),
        _ => LookupResult::Ambiguous(results),
    })
}

/// Result of an unambiguous name lookup.
///
/// Implements [`Display`] for friendly CLI output:
///
/// ```ignore
/// match result {
///     LookupResult::Found(v) => println!("{v}"),
///     LookupResult::Ambiguous(list) => println!("{result}"),
///     LookupResult::NotFound => eprintln!("{result}"),
/// }
/// ```
#[derive(Debug, Clone)]
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

/// A village record with administrative hierarchy and coordinates.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum LocateMethod {
    /// Matched by nearest village centroid (Haversine distance).
    Nearest,
    /// Matched by polygon containment (future).
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
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
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
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
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

/// Reverse-geocode a lat/lon to the full administrative hierarchy.
///
/// Finds the nearest village centroid and parses its administrative code
/// to build the province, city, and district hierarchy.
///
/// # Arguments
///
/// * `conn` - Database connection from [`open_embedded`]
/// * `lat` - Latitude (-90..90)
/// * `lon` - Longitude (-180..180)
///
/// # Example
///
/// ```ignore
/// let conn = wilayah::open()?;
/// if let Some(loc) = wilayah::locate(&conn, -6.1647, 106.8453)? {
///     println!("{loc}");
/// }
/// ```
pub fn locate(conn: &Connection, lat: f64, lon: f64) -> Result<Option<Location>> {
    let mut results = nearest(conn, lat, lon, 1)?;
    let village = match results.pop() {
        Some(v) => v,
        None => return Ok(None),
    };

    let dist_km = village.dist_km.unwrap_or(0.0);

    let parts: Vec<&str> = village.code.split('.').collect();
    if parts.len() != 4 {
        return Ok(None);
    }

    let province_code = parts[0].to_string();
    let city_code = format!("{}.{}", parts[0], parts[1]);
    let district_code = format!("{}.{}.{}", parts[0], parts[1], parts[2]);

    Ok(Some(Location {
        province: AdminLevel {
            code: province_code,
            name: village.province.clone(),
        },
        city: AdminLevel {
            code: city_code,
            name: village.city.clone(),
        },
        district: AdminLevel {
            code: district_code,
            name: village.district.clone(),
        },
        village: village.name,
        village_code: village.code,
        lat: village.lat,
        lon: village.lon,
        dist_km,
        method: LocateMethod::Nearest,
    }))
}
