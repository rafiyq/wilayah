use rusqlite::{functions::FunctionFlags, Connection, Result};
use std::fmt;

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
#[derive(Debug, Clone, serde::Serialize)]
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
