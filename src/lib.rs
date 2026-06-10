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
//!   [`AdminLevel`], [`LocateMethod`]). Use this with `default-features = false`
//!   when you only need the types (e.g., Cloudflare Workers with a different
//!   database backend).
//! - **`raw-sqlite`** — Exposes `Database::conn_guard()` for direct `rusqlite`
//!   access. Using this accessor makes your code dependent on `rusqlite`'s API.

#![deny(missing_docs)]

/// Shared types for Indonesian village location data.
///
/// This module is always available regardless of feature flags. It contains
/// the core data types ([`Village`], [`Location`], [`AdminLevel`], etc.) and
/// the [`location_from_village`] helper for building administrative hierarchies
/// from village codes.
pub mod types;

/// Geographic computations: distance, point-in-polygon, and vertex serialization.
///
/// Contains [`haversine_km`], [`PipResult`], [`point_in_ring`], [`point_in_polygon`],
/// [`bbox`], [`serialize_vertices`], and [`deserialize_vertices`].
pub mod geometry;

#[cfg(feature = "db")]
mod db;

#[cfg(feature = "build-db")]
pub mod builder;

pub use geometry::{
    bbox, deserialize_vertices, haversine_km, point_in_polygon, point_in_ring, serialize_vertices,
    PipResult, EARTH_RADIUS_KM,
};

pub use types::{
    location_from_village, AdminLevel, DataInfo, LocateMethod, Location, LookupResult,
    PrefixResult, Village, CODE_PREFIX_MAX_LIMIT, NEAREST_MAX_LIMIT, SEARCH_MAX_LIMIT,
};

#[cfg(feature = "build-db")]
pub use builder::RingClassification;

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
