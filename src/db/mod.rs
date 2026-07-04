//! Embedded SQLite database for Indonesian village lookups.

mod meta;
mod polygon;
mod query;

use rusqlite::{functions::FunctionFlags, Connection};
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::geometry::haversine_km;
use crate::types::{DataInfo, Location, LookupResult, PrefixResult, Village};

const DB_BYTES: &[u8] = include_bytes!(env!("LOCATION_DB_PATH"));

/// Village columns in the `locations` table (Indonesian SQL names).
///
/// Mapping to English struct fields:
/// `kode` → `code`, `nama` → `name`, `kecamatan` → `district`,
/// `kota` → `city`, `provinsi` → `province`, `lat` → `lat`, `lon` → `lon`.
pub(super) const VILLAGE_COLS: &str = "kode, nama, kecamatan, kota, provinsi, lat, lon";

/// Same columns with `l.` prefix for JOIN queries against the `locations` table.
pub(super) const VILLAGE_COLS_L: &str =
    "l.kode, l.nama, l.kecamatan, l.kota, l.provinsi, l.lat, l.lon";

/// Error type for database operations.
///
/// Wraps internal `rusqlite` errors without exposing the `rusqlite::Error`
/// type in the public API. Implements [`std::error::Error`] and can be
/// converted from `rusqlite::Error` automatically.
#[derive(Debug)]
#[non_exhaustive]
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

#[cfg(feature = "serde")]
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
/// # Polygon containment
///
/// By default, `locate()` uses nearest-centroid matching. Call
/// [`open_with_polygons()`](Database::open_with_polygons) to load a polygon
/// database built with `Pipeline::include_polygons(true)`. When a polygon DB
/// is loaded, `locate()` automatically uses polygon containment when available,
/// falling back to nearest-centroid for villages without polygon data.
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
    poly_conn: Option<Mutex<Connection>>,
}

impl Database {
    fn lock_conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn lock_poly(&self) -> Result<MutexGuard<'_, Connection>> {
        self.poly_conn
            .as_ref()
            .ok_or_else(|| Error {
                inner: rusqlite::Error::InvalidParameterName(
                    "lock_poly called without polygon DB".into(),
                ),
            })
            .map(|conn| conn.lock().unwrap_or_else(|e| e.into_inner()))
    }

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
            poly_conn: None,
        })
    }

    /// Open the embedded location database with an additional polygon database.
    ///
    /// Loads the main database as [`open()`](Database::open) does, then opens
    /// the polygon database from `poly_path`. The polygon database should be
    /// built with `Pipeline::include_polygons(true)` and contains village
    /// boundary geometry for polygon-containment lookups.
    ///
    /// When a polygon database is loaded, [`locate()`](Database::locate) will
    /// use polygon containment when the query point falls inside a village
    /// boundary, returning [`LocateMethod::Contained`]. Villages without
    /// polygon data fall back to nearest-centroid matching.
    ///
    /// # Arguments
    ///
    /// * `poly_path` - Path to the `locations-poly.db` file
    ///
    /// # Example
    ///
    /// ```no_run
    /// let db = wilayah::Database::open_with_polygons("data/locations-poly.db")?;
    /// if let Some(loc) = db.locate(-6.1647, 106.8453)? {
    ///     println!("Method: {}", loc.method);
    /// }
    /// # Ok::<_, wilayah::Error>(())
    /// ```
    pub fn open_with_polygons(poly_path: &str) -> Result<Self> {
        let mut db = Self::open()?;
        let poly_conn = Connection::open(poly_path)?;
        db.poly_conn = Some(Mutex::new(poly_conn));
        Ok(db)
    }

    /// Returns `true` if a polygon database has been loaded.
    ///
    /// When `true`, [`locate()`](Database::locate) will use polygon containment
    /// when the query point falls inside a village boundary.
    pub fn has_polygons(&self) -> bool {
        self.poly_conn.is_some()
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
        query::nearest(&self.lock_conn(), lat, lon, limit)
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
        query::search(&self.lock_conn(), query, limit)
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
        query::search_unique(&self.lock_conn(), query)
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
        query::by_code(&self.lock_conn(), code)
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
        query::by_code_prefix(&self.lock_conn(), prefix, limit, offset)
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
        if self.poly_conn.is_some() {
            let candidates = {
                let poly = self.lock_poly()?;
                polygon::query_polygon_candidates(&poly, lat, lon)?
            };
            if let Some(loc) =
                polygon::locate_contained(&candidates, &self.lock_conn(), lat, lon, query::by_id)?
            {
                return Ok(Some(loc));
            }
            return query::locate_nearest(&self.lock_conn(), lat, lon);
        }
        query::locate_nearest(&self.lock_conn(), lat, lon)
    }

    /// Get metadata about the embedded location database.
    ///
    /// Reads the `db_meta` table for decree, source, build date, and
    /// village count. Returns default values if the table is missing
    /// or keys are absent.
    pub fn data_info(&self) -> DataInfo {
        meta::data_info_from_conn(&self.lock_conn())
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
            self.lock_conn()
                .query_row("SELECT COUNT(*) FROM locations", [], |row| row.get(0))?;
        u32::try_from(count).map_err(|e| Error {
            inner: rusqlite::Error::InvalidParameterName(format!("village count overflow: {}", e)),
        })
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
        self.lock_conn()
    }
}

static CACHED_DATA_INFO: OnceLock<DataInfo> = OnceLock::new();

pub(crate) fn cached_data_info() -> &'static DataInfo {
    CACHED_DATA_INFO.get_or_init(|| {
        Database::open()
            .map(|db| db.data_info())
            .unwrap_or_default()
    })
}
