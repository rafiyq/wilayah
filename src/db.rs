use rusqlite::{functions::FunctionFlags, Connection};
#[cfg(feature = "raw-sqlite")]
use std::sync::MutexGuard;
use std::sync::{Mutex, OnceLock};

use crate::types::{
    haversine_km, location_from_village, DataInfo, Location, LookupResult, PrefixResult, Village,
};

const DB_BYTES: &[u8] = include_bytes!(env!("LOCATION_DB_PATH"));

/// Error type for database operations.
///
/// Wraps internal `rusqlite` errors without exposing the `rusqlite::Error`
/// type in the public API. Implements [`std::error::Error`] and can be
/// converted from `rusqlite::Error` automatically.
#[derive(Debug)]
pub struct Error {
    inner: rusqlite::Error,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.inner)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error { inner: e }
    }
}

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// Result type for database operations.
pub type Result<T> = std::result::Result<T, Error>;

/// An open handle to the embedded wilayah location database.
///
/// Wraps an internal SQLite connection and provides methods for querying
/// Indonesian village data. The `rusqlite::Connection` is kept private so
/// that the crate's public API is independent of the `rusqlite` major version.
///
/// # Thread safety
///
/// `Database` is `Send + Sync`. The internal `rusqlite::Connection` is wrapped
/// in a `Mutex`, allowing safe shared access across threads (e.g., via
/// `Arc<Database>` in async servers).
///
/// # Example
///
/// ```
/// let db = wilayah::Database::open()?;
/// let results = db.find_nearest(-6.1647, 106.8453, 5)?;
/// # Ok::<_, wilayah::Error>(())
/// ```
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open the embedded location database.
    ///
    /// Loads the ~20 MB SQLite database from the compiled binary into memory
    /// using SQLite's online backup API. The database contains village records
    /// with spatial (RTree) and full-text (FTS5) indexes.
    ///
    /// # Example
    ///
    /// ```
    /// let db = wilayah::Database::open()?;
    /// # Ok::<_, wilayah::Error>(())
    /// ```
    pub fn open() -> Result<Self> {
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

        Ok(Database {
            conn: Mutex::new(conn),
        })
    }

    /// Find the nearest villages to a given latitude/longitude.
    ///
    /// Uses a SQLite RTree spatial index for fast bounding-box filtering,
    /// followed by Haversine distance calculation to find the closest villages.
    /// The search progressively expands the search radius until results are
    /// found or the full globe has been searched.
    ///
    /// # Arguments
    ///
    /// * `lat` - Latitude (-90..90)
    /// * `lon` - Longitude (-180..180)
    /// * `limit` - Maximum number of results to return (clamped to 1..20)
    ///
    /// # Example
    ///
    /// ```
    /// let db = wilayah::Database::open()?;
    /// let results = db.find_nearest(-6.1647, 106.8453, 5)?;
    /// for v in results {
    ///     println!("{} ({:.1} km)", v.name, v.dist_km.unwrap());
    /// }
    /// # Ok::<_, wilayah::Error>(())
    /// ```
    pub fn find_nearest(&self, lat: f64, lon: f64, limit: usize) -> Result<Vec<Village>> {
        nearest(&self.conn.lock().unwrap(), lat, lon, limit)
    }

    /// Search for villages by name.
    ///
    /// Uses FTS5 full-text search matching against village name, district,
    /// city, and province. Supports partial matches and returns results
    /// ranked by BM25.
    ///
    /// For disambiguation, include city or province in the query:
    /// `find_by_name("kemayoran jakarta")` returns only villages in Jakarta.
    ///
    /// # Arguments
    ///
    /// * `query` - Search query (e.g., `"kemayoran"` or `"kemayoran jakarta"`)
    /// * `limit` - Maximum number of results to return (clamped to 1..100)
    ///
    /// # Example
    ///
    /// ```
    /// let db = wilayah::Database::open()?;
    /// let results = db.find_by_name("kemayoran jakarta", 10)?;
    /// for v in results {
    ///     println!("{} in {}, {}", v.name, v.district, v.province);
    /// }
    /// # Ok::<_, wilayah::Error>(())
    /// ```
    pub fn find_by_name(&self, query: &str, limit: usize) -> Result<Vec<Village>> {
        search(&self.conn.lock().unwrap(), query, limit)
    }

    /// Search for a unique village by name.
    ///
    /// Returns [`LookupResult::Found`] if exactly one match exists,
    /// [`LookupResult::Ambiguous`] with up to 20 candidates if multiple match,
    /// or [`LookupResult::NotFound`] if no match exists.
    ///
    /// # Arguments
    ///
    /// * `query` - Search query (e.g., `"kemayoran"` or `"kemayoran jakarta"`)
    ///
    /// # Example: exact match
    ///
    /// ```
    /// let db = wilayah::Database::open()?;
    /// let result = db.find_by_name_unique("abadijaya")?;
    /// if let wilayah::LookupResult::Found(v) = result {
    ///     println!("Found: {} in {}", v.name, v.city);
    /// }
    /// # Ok::<_, wilayah::Error>(())
    /// ```
    pub fn find_by_name_unique(&self, query: &str) -> Result<LookupResult> {
        search_unique(&self.conn.lock().unwrap(), query)
    }

    /// Find a village by its BMKG-compatible administrative code.
    ///
    /// Returns `None` if the code is not found in the database.
    ///
    /// # Example
    ///
    /// ```
    /// let db = wilayah::Database::open()?;
    /// let v = db.find_by_code("31.71.03.1001")?;
    /// assert!(v.is_some());
    /// # Ok::<_, wilayah::Error>(())
    /// ```
    pub fn find_by_code(&self, code: &str) -> Result<Option<Village>> {
        by_code(&self.conn.lock().unwrap(), code)
    }

    /// Find all villages matching an administrative code prefix with pagination.
    ///
    /// Useful for listing all villages in a kecamatan (`"31.71.03"`),
    /// kabupaten (`"31.71"`), or province (`"31"`). Returns a paginated
    /// result with total count and a `has_more` flag.
    ///
    /// # Arguments
    ///
    /// * `prefix` - Code prefix (e.g., `"31.71.03"`, `"31.71"`, `"31"`)
    /// * `limit` - Maximum number of results per page (clamped to 1..1000)
    /// * `offset` - Number of results to skip (for pagination)
    ///
    /// # Example
    ///
    /// ```
    /// let db = wilayah::Database::open()?;
    /// let result = db.find_by_code_prefix("31.71.03", 100, 0)?;
    /// assert!(!result.villages.is_empty());
    /// # Ok::<_, wilayah::Error>(())
    /// ```
    pub fn find_by_code_prefix(
        &self,
        prefix: &str,
        limit: usize,
        offset: usize,
    ) -> Result<PrefixResult> {
        by_code_prefix(&self.conn.lock().unwrap(), prefix, limit, offset)
    }

    /// Reverse-geocode a lat/lon to the full administrative hierarchy.
    ///
    /// Finds the nearest village centroid and returns the complete
    /// administrative hierarchy: province, city/regency, district, and
    /// village with their codes and names.
    ///
    /// # Arguments
    ///
    /// * `lat` - Latitude (-90..90)
    /// * `lon` - Longitude (-180..180)
    ///
    /// # Example
    ///
    /// ```
    /// let db = wilayah::Database::open()?;
    /// if let Some(loc) = db.locate(-6.1647, 106.8453)? {
    ///     assert_eq!(loc.province.code, "31");
    ///     assert!(loc.city.name.contains("Jakarta"));
    /// }
    /// # Ok::<_, wilayah::Error>(())
    /// ```
    pub fn locate(&self, lat: f64, lon: f64) -> Result<Option<Location>> {
        locate(&self.conn.lock().unwrap(), lat, lon)
    }

    /// Get metadata about the embedded location database.
    ///
    /// Reads the `db_meta` table for decree, source, build date, and
    /// village count. Returns default values if the table is missing
    /// or keys are absent.
    pub fn data_info(&self) -> DataInfo {
        data_info_from_conn(&self.conn.lock().unwrap())
    }

    /// Get the total number of villages in the database.
    ///
    /// # Example
    ///
    /// ```
    /// let db = wilayah::Database::open()?;
    /// let count = db.village_count()?;
    /// assert!(count > 80000);
    /// # Ok::<_, wilayah::Error>(())
    /// ```
    pub fn village_count(&self) -> Result<u32> {
        let count: i64 =
            self.conn
                .lock()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM locations", [], |row| row.get(0))?;
        Ok(count as u32)
    }
}

/// Get the underlying `rusqlite::Connection`.
///
/// This is intended for advanced use cases that need direct SQLite access
/// (e.g., custom queries, attaching additional databases). Using this
/// accessor makes your code dependent on `rusqlite`'s API, which may
/// change across major versions independently of `wilayah`'s semver.
///
/// Only available with the `raw-sqlite` feature flag.
#[cfg(feature = "raw-sqlite")]
impl Database {
    /// Acquire the internal `MutexGuard` holding the `rusqlite::Connection`.
    ///
    /// The returned guard derefs to `&Connection`, and the lock is held for
    /// the guard's lifetime, preventing concurrent access.
    ///
    /// See the [feature flag documentation](#feature-flags) for caveats.
    pub fn conn_guard(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }
}

fn query_meta(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM db_meta WHERE key = ?1", [key], |row| {
        row.get(0)
    })
    .ok()
}

fn data_info_from_conn(conn: &Connection) -> DataInfo {
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

static CACHED_DATA_INFO: OnceLock<DataInfo> = OnceLock::new();

pub(crate) fn cached_data_info() -> &'static DataInfo {
    CACHED_DATA_INFO.get_or_init(|| {
        let db = Database::open().expect("failed to open embedded database for metadata");
        db.data_info()
    })
}

fn nearest(conn: &Connection, lat: f64, lon: f64, limit: usize) -> Result<Vec<Village>> {
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

        let results: Vec<Village> = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)?;

        if results.len() >= limit {
            return Ok(results);
        }
    }

    Ok(vec![])
}

fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<Village>> {
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

    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Error::from)
}

fn by_code(conn: &Connection, code: &str) -> Result<Option<Village>> {
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
        Some(Err(e)) => Err(Error::from(e)),
        None => Ok(None),
    }
}

fn by_code_prefix(
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
    let villages: Vec<Village> = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Error::from)?;

    let has_more = offset + villages.len() < total;

    Ok(PrefixResult {
        villages,
        total,
        has_more,
    })
}

fn search_unique(conn: &Connection, query: &str) -> Result<LookupResult> {
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
    let results: Vec<_> = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Error::from)?;

    Ok(match results.len() {
        0 => LookupResult::NotFound,
        1 => LookupResult::Found(results.into_iter().next().unwrap()),
        _ => LookupResult::Ambiguous(results),
    })
}

fn locate(conn: &Connection, lat: f64, lon: f64) -> Result<Option<Location>> {
    let mut results = nearest(conn, lat, lon, 1)?;
    let village = match results.pop() {
        Some(v) => v,
        None => return Ok(None),
    };

    let dist_km = village.dist_km.unwrap_or(0.0);
    Ok(location_from_village(&village, dist_km))
}
