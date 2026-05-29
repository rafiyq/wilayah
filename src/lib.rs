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
//! use wilayah::Database;
//!
//! let db = Database::open().expect("database");
//! let nearest = db.find_nearest(-6.1647, 106.8453, 5).expect("query");
//! assert!(!nearest.is_empty());
//! ```
//!
//! # Data
//!
//! Sourced from the official Kemendagri (Ministry of Home Affairs) PDF
//! publication of all Indonesian villages, combined with village polygon
//! boundaries from BIG (Badan Informasi Geospasial) ArcGIS service. The data is
//! processed through a reproducible build pipeline to produce a SQLite database
//! with RTree spatial index and FTS5 full-text search. The database is embedded
//! into the binary at compile time via a build script.
//!
//! On first `cargo build`, the build script downloads a pre-built database
//! from the GitHub Releases. Subsequent builds reuse the cached database
//! located at `data/locations.db`.
//!
//! To build from scratch, run:
//!
//! ```bash
//! cargo run --example build_db --features build-db
//! ```
//!
//! # Feature flags
//!
//! - **`db`** *(enabled by default)* — Embedded SQLite database via `rusqlite`.
//!   Enables [`Database`], [`Error`], and all query methods.
//! - **`build-db`** — Pipeline for building the database from source data.
//! - **`types`** — *(always available)* Shared types ([`Village`], [`Location`],
//!   [`AdminLevel`], [`LocateMethod`]) and [`haversine_km()`]. Use this with
//!   `default-features = false` when you only need the types (e.g., Cloudflare
//!   Workers with a different database backend).
//! - **`raw-sqlite`** — Exposes `Database::conn()` for direct `rusqlite`
//!   access. Using this accessor makes your code dependent on `rusqlite`'s API.

#![deny(missing_docs)]

/// Shared types and utilities for Indonesian village location data.
///
/// This module is always available regardless of feature flags. It contains
/// the core data types ([`Village`], [`Location`], [`AdminLevel`], etc.),
/// the [`haversine_km`] distance function, and the [`location_from_village`]
/// helper for building administrative hierarchies from village codes.
pub mod types;

#[cfg(feature = "db")]
mod db;

#[cfg(feature = "build-db")]
pub mod builder;

pub use types::{
    haversine_km, location_from_village, AdminLevel, DataInfo, LocateMethod, Location,
    LookupResult, PrefixResult, Village,
};

#[cfg(feature = "db")]
pub use db::{Database, Error};

/// Get the version of this crate.
///
/// Returns the `Cargo.toml` version string.
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Get metadata about the embedded location database.
///
/// Returns source, decree, village count, and build timestamp information
/// stored in the `db_meta` table of the embedded database. The result is
/// cached after the first call.
///
/// # Example
///
/// ```
/// let info = wilayah::data_info();
/// // village_count and build_date are 0 if the DB predates the db_meta table
/// if info.village_count > 0 {
///     assert!(info.village_count > 80000);
/// }
/// ```
#[cfg(feature = "db")]
pub fn data_info() -> DataInfo {
    db::cached_data_info().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_db() {
        let db = Database::open().expect("should open embedded database");
        let count = db.village_count().expect("should count villages");
        assert!(count > 80000, "expected >80k villages, got {count}");
    }

    #[test]
    fn test_data_info() {
        let info = data_info();
        if info.village_count > 0 {
            assert!(info.village_count > 80000);
        }
        if info.build_date > 0 {
            assert!(!info.source.is_empty());
            assert!(!info.decree.is_empty());
            assert!(
                !info.decree.contains("unknown"),
                "decree should be from DB, not 'unknown': {}",
                info.decree
            );
        }
    }

    #[test]
    fn test_data_info_via_database() {
        let db = Database::open().unwrap();
        let info = db.data_info();
        assert_eq!(info, data_info());
    }

    #[test]
    fn test_version() {
        assert_eq!(version(), "0.5.0");
    }

    #[test]
    fn test_nearest_jakarta() {
        let db = Database::open().unwrap();
        let results = db.find_nearest(-6.1647, 106.8453, 1).unwrap();
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
        let db = Database::open().unwrap();
        let results = db.find_nearest(-2.5, 140.0, 1).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].province.contains("Papua"));
    }

    #[test]
    fn test_search() {
        let db = Database::open().unwrap();
        let results = db.find_by_name("kemayoran", 5).unwrap();
        assert!(!results.is_empty(), "should find Kemayoran");
        assert!(results
            .iter()
            .any(|v| v.name.to_lowercase().contains("kemayoran")));
    }

    #[test]
    fn test_search_qualified() {
        let db = Database::open().unwrap();
        let results = db.find_by_name("kemayoran jakarta", 5).unwrap();
        assert!(!results.is_empty(), "should find Kemayoran Jakarta");
        assert!(results.iter().all(|v| v.city.contains("Jakarta")));
    }

    #[test]
    fn test_unique_found() {
        let db = Database::open().unwrap();
        let result = db.find_by_name_unique("abadijaya").unwrap();
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
        let db = Database::open().unwrap();
        let result = db.find_by_name_unique("sukamaju").unwrap();
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
    fn test_find_by_code() {
        let db = Database::open().unwrap();
        let v = db.find_by_code("31.71.03.1001").unwrap();
        assert!(v.is_some(), "31.71.03.1001 should exist");
        let v = v.unwrap();
        assert_eq!(v.name, "Kemayoran");
        assert_eq!(v.district, "Kemayoran");
        assert_eq!(v.city, "Kota Administrasi Jakarta Pusat");
        assert_eq!(v.province, "Provinsi Daerah Khusus Ibukota Jakarta");
    }

    #[test]
    fn test_find_by_code_not_found() {
        let db = Database::open().unwrap();
        let v = db.find_by_code("99.99.99.9999").unwrap();
        assert!(v.is_none());
    }

    #[test]
    fn test_find_by_code_prefix_kecamatan() {
        let db = Database::open().unwrap();
        let result = db.find_by_code_prefix("31.71.03", 100, 0).unwrap();
        assert!(
            !result.villages.is_empty(),
            "should find villages in kecamatan 31.71.03"
        );
        assert!(result
            .villages
            .iter()
            .all(|v| v.code.starts_with("31.71.03")));
        assert!(result.villages.iter().all(|v| v.district == "Kemayoran"));
        assert_eq!(result.total, result.villages.len());
        assert!(!result.has_more);
    }

    #[test]
    fn test_find_by_code_prefix_kabupaten() {
        let db = Database::open().unwrap();
        let result = db.find_by_code_prefix("31.71", 500, 0).unwrap();
        assert!(
            !result.villages.is_empty(),
            "should find villages in kabupaten 31.71"
        );
        assert!(result.villages.iter().all(|v| v.code.starts_with("31.71")));
        assert!(result.total > 0);
        assert_eq!(result.has_more, result.villages.len() < result.total);
    }

    #[test]
    fn test_find_by_code_prefix_not_found() {
        let db = Database::open().unwrap();
        let result = db.find_by_code_prefix("99.99.99", 100, 0).unwrap();
        assert!(result.villages.is_empty());
        assert_eq!(result.total, 0);
        assert!(!result.has_more);
    }

    #[test]
    fn test_unique_not_found() {
        let db = Database::open().unwrap();
        let result = db.find_by_name_unique("zzzznonexistent").unwrap();
        assert!(
            matches!(result, LookupResult::NotFound),
            "should be not found, got {:?}",
            result
        );
    }

    #[test]
    fn test_locate_jakarta() {
        let db = Database::open().unwrap();
        let loc = db
            .locate(-6.1647, 106.8453)
            .unwrap()
            .expect("should locate Jakarta");
        assert_eq!(loc.province.code, "31");
        assert!(loc.city.name.contains("Jakarta"));
        assert!(loc.district.name.len() > 0);
        assert!(loc.village.len() > 0);
        assert!(loc.village_code.contains('.'));
        assert!(loc.dist_km < 5.0);
        assert_eq!(loc.method, LocateMethod::Nearest);
    }

    #[test]
    fn test_locate_display() {
        let db = Database::open().unwrap();
        let loc = db
            .locate(-6.1647, 106.8453)
            .unwrap()
            .expect("should locate Jakarta");
        let s = format!("{loc}");
        assert!(s.contains(&loc.province.code));
        assert!(s.contains(&loc.village));
    }

    #[test]
    fn test_admin_level_display() {
        let level = AdminLevel {
            code: "31.71".into(),
            name: "Jakarta".into(),
        };
        assert_eq!(format!("{level}"), "31.71 Jakarta");
    }

    #[test]
    fn test_locate_method_display() {
        assert_eq!(format!("{}", LocateMethod::Nearest), "nearest");
        assert_eq!(format!("{}", LocateMethod::Contained), "contained");
    }

    #[test]
    fn test_haversine_km() {
        let d = haversine_km(-6.1647, 106.8453, -6.1647, 106.8453);
        assert!(d.abs() < 0.001, "same point should be 0 km, got {d}");
        let d = haversine_km(-6.1647, 106.8453, -6.2, 106.8);
        assert!(d > 0.0 && d < 50.0, "nearby point should be close, got {d}");
    }

    #[test]
    fn test_location_from_village() {
        let v = Village {
            code: "31.71.03.1001".into(),
            name: "Kemayoran".into(),
            district: "Kemayoran".into(),
            city: "Jakarta Pusat".into(),
            province: "DKI Jakarta".into(),
            lat: -6.1647,
            lon: 106.8453,
            dist_km: None,
        };
        let loc = location_from_village(&v, 1.5).expect("should parse valid code");
        assert_eq!(loc.province.code, "31");
        assert_eq!(loc.city.code, "31.71");
        assert_eq!(loc.district.code, "31.71.03");
        assert_eq!(loc.village_code, "31.71.03.1001");
        assert_eq!(loc.dist_km, 1.5);
        assert_eq!(loc.method, LocateMethod::Nearest);
    }

    #[test]
    fn test_location_from_village_bad_code() {
        let v = Village {
            code: "invalid".into(),
            name: "Test".into(),
            district: "Test".into(),
            city: "Test".into(),
            province: "Test".into(),
            lat: 0.0,
            lon: 0.0,
            dist_km: None,
        };
        assert!(location_from_village(&v, 0.0).is_none());
    }

    #[test]
    fn test_error_display() {
        let db = Database::open().unwrap();
        let result = db.find_by_code("31.71.03.1001");
        assert!(result.is_ok());
    }

    #[test]
    fn test_database_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Database>();
    }
}
