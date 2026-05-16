//! Location lookup for Indonesian villages by GPS coordinates or name.
//!
//! Returns BMKG-compatible `adm4` administrative codes (e.g., `31.71.03.1001`)
//! for 82,689 villages across Indonesia, based on Kepmendagri No 300.2.2-2430
//! Tahun 2025.
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
//! [cahyadsn/wilayah_boundaries](https://github.com/cahyadsn/wilayah_boundaries),
//! based on official Kemendagri administrative codes with pre-computed village
//! centroids from BIG (Badan Informasi Geospasial) polygon boundaries.
//!
//! On first `cargo build`, the raw data is downloaded from GitHub and a SQLite
//! database with RTree spatial index and FTS5 full-text search is built. The
//! database is embedded into the binary at compile time. Subsequent builds
//! reuse the cached database.

mod db;

pub use db::{nearest, open_embedded, search, Village};

/// Open the embedded database.
///
/// Loads the 20 MB SQLite database from the compiled binary into memory using
/// SQLite's online backup API. The database contains 82,689 villages with
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
/// by Haversine distance calculation to find the closest villages.
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
/// let results = wilayah::find_by_name(&conn, "kemayoran", 10)?;
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

/// Get the total number of villages in the database.
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
    fn test_search() {
        let conn = open().unwrap();
        let results = find_by_name(&conn, "kemayoran", 5).unwrap();
        assert!(!results.is_empty(), "should find Kemayoran");
        assert!(results
            .iter()
            .any(|v| v.name.to_lowercase().contains("kemayoran")));
    }
}
