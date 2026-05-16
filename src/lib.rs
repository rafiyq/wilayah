//! Location lookup for Indonesian villages by GPS coordinates or name.
//!
//! Returns BMKG-compatible `adm4` administrative codes (e.g., `31.71.03.1001`)
//! for 82,689 villages across Indonesia, based on official Kemendagri
//! administrative codes with pre-computed village centroids from BIG (Badan
//! Informasi Geospasial) polygon boundaries.
//!
//! # Quick start
//!
//! ```
//! use wilayah;
//!
//! let conn = wilayah::open().expect("database");
//! let nearest = wilayah::find_nearest(&conn, -6.1647, 106.8453, 5).expect("query");
//! assert!(!nearest.is_empty());
//! ```
//!
//! # Data
//!
//! Sourced from [cahyadsn/wilayah](https://github.com/cahyadsn/wilayah) and
//! [cahyadsn/wilayah_boundaries](https://github.com/cahyadsn/wilayah_boundaries).
//!
//! On first `cargo build`, the raw data is downloaded from GitHub and a SQLite
//! database with RTree spatial index and FTS5 full-text search is built. The
//! database is embedded into the binary at compile time. Subsequent builds
//! reuse the cached database.
//!
//! To build offline, set `WILAYAH_DATA_DIR` to a directory containing
//! `wilayah.sql` and `kel/*.sql` files downloaded from the upstream repos.

#![deny(missing_docs)]

mod db;

pub use db::{nearest, open_embedded, search, search_unique, LookupResult, Village};

/// Metadata about the embedded location database.
///
/// Returned by [`data_info()`]. Contains information about the data source,
/// the government decree it's based on, the number of villages, and when
/// the database was built.
#[derive(Debug, Clone)]
pub struct DataInfo {
    /// The upstream data source (e.g., `"cahyadsn/wilayah_boundaries"`).
    pub source: &'static str,
    /// The government decree this data is based on
    /// (e.g., `"Kepmendagri No 300.2.2-2430 Tahun 2025"`).
    pub decree: &'static str,
    /// The number of villages in the database.
    pub village_count: u32,
    /// Unix timestamp (seconds since epoch) of when this database was built.
    pub build_date: u64,
}

/// Get the version of this crate.
///
/// Returns the `Cargo.toml` version string.
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Get metadata about the embedded location database.
///
/// Returns source, decree, village count, and build timestamp information
/// compiled into the binary.
///
/// # Example
///
/// ```
/// let info = wilayah::data_info();
/// assert!(info.village_count > 80000);
/// assert!(info.build_date > 0);
/// ```
pub fn data_info() -> DataInfo {
    DataInfo {
        source: env!("WILAYAH_DATA_SOURCE"),
        decree: env!("WILAYAH_DATA_DECREE"),
        village_count: env!("WILAYAH_VILLAGE_COUNT").parse().unwrap_or(0),
        build_date: env!("WILAYAH_BUILD_DATE").parse().unwrap_or(0),
    }
}

/// Open the embedded database.
///
/// Loads the ~20 MB SQLite database from the compiled binary into memory using
/// SQLite's online backup API. The database contains village records with
/// spatial (RTree) and full-text (FTS5) indexes.
///
/// # Example
///
/// ```
/// let conn = wilayah::open()?;
/// # Ok::<_, rusqlite::Error>(())
/// ```
pub fn open() -> rusqlite::Result<rusqlite::Connection> {
    db::open_embedded()
}

/// Find the nearest villages to a given latitude/longitude.
///
/// Uses a SQLite RTree spatial index for fast bounding-box filtering, followed
/// by Haversine distance calculation to find the closest villages. The search
/// progressively expands the search radius until results are found or the full
/// globe has been searched.
///
/// # Arguments
///
/// * `conn` - Database connection from [`open()`]
/// * `lat` - Latitude (-90..90)
/// * `lon` - Longitude (-180..180)
/// * `limit` - Maximum number of results to return (clamped to 1..20)
///
/// # Example
///
/// ```
/// let conn = wilayah::open()?;
/// let results = wilayah::find_nearest(&conn, -6.1647, 106.8453, 5)?;
/// for v in results {
///     println!("{} ({:.1} km)", v.name, v.dist_km.unwrap());
/// }
/// # Ok::<_, rusqlite::Error>(())
/// ```
///
/// # Edge case: Papua coordinates
///
/// ```
/// let conn = wilayah::open()?;
/// let results = wilayah::find_nearest(&conn, -2.5, 140.0, 1)?;
/// assert!(!results.is_empty());
/// assert!(results[0].province.contains("Papua"));
/// # Ok::<_, rusqlite::Error>(())
/// ```
pub fn find_nearest(
    conn: &rusqlite::Connection,
    lat: f64,
    lon: f64,
    limit: usize,
) -> rusqlite::Result<Vec<Village>> {
    db::nearest(conn, lat, lon, limit)
}

/// Search for villages by name.
///
/// Uses FTS5 full-text search matching against village name, district, city,
/// and province. Supports partial matches and returns results ranked by BM25.
///
/// For disambiguation, include city or province in the query:
/// `find_by_name("kemayoran jakarta")` returns only villages in Jakarta.
///
/// # Arguments
///
/// * `conn` - Database connection from [`open()`]
/// * `query` - Search query (e.g., `"kemayoran"` or `"kemayoran jakarta"`)
/// * `limit` - Maximum number of results to return (clamped to 1..100)
///
/// # Example
///
/// ```
/// let conn = wilayah::open()?;
/// let results = wilayah::find_by_name(&conn, "kemayoran jakarta", 10)?;
/// for v in results {
///     println!("{} in {}, {}", v.name, v.district, v.province);
/// }
/// # Ok::<_, rusqlite::Error>(())
/// ```
pub fn find_by_name(
    conn: &rusqlite::Connection,
    query: &str,
    limit: usize,
) -> rusqlite::Result<Vec<Village>> {
    db::search(conn, query, limit)
}

/// Search for a unique village by name.
///
/// Returns [`LookupResult::Found`] if exactly one match exists,
/// [`LookupResult::Ambiguous`] with up to 20 candidates if multiple match,
/// or [`LookupResult::NotFound`] if no match exists.
///
/// This is useful for callers that need unambiguous results. For example,
/// a CLI tool can show an error with candidate list when the result is
/// ambiguous, rather than silently picking the wrong village.
///
/// # Arguments
///
/// * `conn` - Database connection from [`open()`]
/// * `query` - Search query (e.g., `"kemayoran"` or `"kemayoran jakarta"`)
///
/// # Example: exact match
///
/// ```
/// let conn = wilayah::open()?;
/// let result = wilayah::find_by_name_unique(&conn, "abadijaya")?;
/// if let wilayah::LookupResult::Found(v) = result {
///     println!("Found: {} in {}", v.name, v.city);
/// }
/// # Ok::<_, rusqlite::Error>(())
/// ```
///
/// # Example: ambiguous name
///
/// ```
/// let conn = wilayah::open()?;
/// // "sukamaju" exists in many villages across Indonesia
/// let result = wilayah::find_by_name_unique(&conn, "sukamaju")?;
/// assert!(matches!(result, wilayah::LookupResult::Ambiguous(_)));
/// # Ok::<_, rusqlite::Error>(())
/// ```
pub fn find_by_name_unique(
    conn: &rusqlite::Connection,
    query: &str,
) -> rusqlite::Result<LookupResult> {
    db::search_unique(conn, query)
}

/// Get the total number of villages in the database.
///
/// # Example
///
/// ```
/// let conn = wilayah::open()?;
/// let count = wilayah::village_count(&conn)?;
/// assert!(count > 80000);
/// # Ok::<_, rusqlite::Error>(())
/// ```
pub fn village_count(conn: &rusqlite::Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM locations", [], |row| row.get(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_db() {
        let conn = open().expect("should open embedded database");
        let count = village_count(&conn).expect("should count villages");
        assert!(count > 80000, "expected >80k villages, got {count}");
    }

    #[test]
    fn test_data_info() {
        let info = data_info();
        assert!(info.village_count > 80000);
        assert!(info.build_date > 0);
        assert!(!info.source.is_empty());
        assert!(!info.decree.is_empty());
    }

    #[test]
    fn test_version() {
        assert_eq!(version(), "0.1.0");
    }

    #[test]
    fn test_nearest_jakarta() {
        let conn = open().unwrap();
        let results = find_nearest(&conn, -6.1647, 106.8453, 1).unwrap();
        assert_eq!(results.len(), 1);
        let v = &results[0];
        assert!(
            v.dist_km.unwrap() < 5.0,
            "should be within 5km of Jakarta center"
        );
        assert_eq!(
            v.city, "Kota Administrasi Jakarta Pusat",
            "expected Jakarta Pusat, got {}",
            v.city
        );
    }

    #[test]
    fn test_nearest_papua() {
        let conn = open().unwrap();
        let results = find_nearest(&conn, -2.5, 140.0, 1).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].province.contains("Papua"));
    }

    #[test]
    fn test_search() {
        let conn = open().unwrap();
        let results = find_by_name(&conn, "kemayoran", 5).unwrap();
        assert!(!results.is_empty(), "should find Kemayoran");
        assert!(results
            .iter()
            .any(|v| v.name.to_lowercase().contains("kemayoran")));
    }

    #[test]
    fn test_search_qualified() {
        let conn = open().unwrap();
        let results = find_by_name(&conn, "kemayoran jakarta", 5).unwrap();
        assert!(!results.is_empty(), "should find Kemayoran Jakarta");
        assert!(results.iter().all(|v| v.city.contains("Jakarta")));
    }

    #[test]
    fn test_unique_found() {
        let conn = open().unwrap();
        let result = find_by_name_unique(&conn, "abadijaya").unwrap();
        assert!(
            matches!(result, LookupResult::Found(_)),
            "expected Found, got {:?}",
            result
        );
        if let LookupResult::Found(v) = result {
            assert_eq!(v.name, "Abadijaya");
        }
    }

    #[test]
    fn test_unique_ambiguous() {
        let conn = open().unwrap();
        let result = find_by_name_unique(&conn, "sukamaju").unwrap();
        assert!(
            matches!(result, LookupResult::Ambiguous(_)),
            "sukamaju should be ambiguous, got {:?}",
            result
        );
        if let LookupResult::Ambiguous(results) = result {
            assert!(results.len() > 1, "should have multiple matches");
        }
    }

    #[test]
    fn test_unique_not_found() {
        let conn = open().unwrap();
        let result = find_by_name_unique(&conn, "zzzznonexistent").unwrap();
        assert!(
            matches!(result, LookupResult::NotFound),
            "should be not found, got {:?}",
            result
        );
    }
}
