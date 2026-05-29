# Changelog

All notable changes to this project will be documented in this file.

## 0.5.0 - 2026-05-29

### Added

- `Database` struct — wraps an internal SQLite connection, hiding `rusqlite` from the public API
- `wilayah::Error` — custom error type that wraps `rusqlite::Error` without exposing it
- `wilayah::Result<T>` — type alias for `std::result::Result<T, Error>`
- `raw-sqlite` feature flag — exposes `Database::conn()` for direct `rusqlite::Connection` access
- `types` module — always available, contains shared types independent of `rusqlite`
- `Serialize` derive on `PrefixResult`
- Cloudflare Worker example with D1 backend (`examples/cloudflare-worker/`)
- `deploy-worker.yml` GitHub Actions workflow for Worker deployment
- `.gitignore` for cloudflare-worker directory and `.dev.vars`

### Changed

- **BREAKING**: All query functions now take `&self` on `Database` instead of `&rusqlite::Connection` as the first parameter
- **BREAKING**: All query functions now return `wilayah::Result<T>` instead of `rusqlite::Result<T>`
- **BREAKING**: `open()` renamed to `Database::open()` and returns `Result<Database>` instead of `rusqlite::Result<Connection>`
- **BREAKING**: `village_count()` now returns `Result<u32>` instead of `Result<i64>` (consistent with `DataInfo.village_count`)
- `data_info()` free function now internally uses `Database::open()` instead of `Connection::open_in_memory()` directly
- `Database::data_info()` method added as the preferred way to get metadata
- CI workflow now runs `cargo fmt --check --all` and `cargo test --features raw-sqlite`
- Integration tests gated behind `raw-sqlite` feature (they use `Database::conn()`)

### Removed

- **BREAKING**: `rusqlite::Connection` is no longer part of the public API (use `Database` instead, or `raw-sqlite` feature for escape hatch)
- **BREAKING**: `rusqlite::Result` and `rusqlite::Error` no longer appear in public signatures

## 0.4.0 - 2026-05-27

### Added

- `find_by_code_prefix` now supports pagination with `offset` parameter and returns `PrefixResult` containing `total` and `has_more` (breaking)
- `data_info_from_conn()` — get database metadata from an existing connection
- `db_meta` table in builder-built databases — stores decree, source, build date, and village count
- `PartialEq` derive on `Village` and `DataInfo`
- Unit tests for core builder functions (`parse_section_header`, `extract_village_name`, `polygon_centroid`, `compute_centroid`, `merge_villages`, `parse_villages`, `build_db`)
- `locate()` — reverse-geocode lat/lon to full administrative hierarchy (province, city, district, village)
- `Location` struct — complete administrative hierarchy with codes, names, coordinates, distance, and method
- `AdminLevel` struct — single administrative level with code and name
- `LocateMethod` enum — `Nearest` (centroid-based) and `Contained` (future polygon-based)
- `Display` impl for `Location`, `AdminLevel`, `LocateMethod`
- `GET /locate` endpoint in HTTP server example

### Changed

- `find_by_code_prefix` return type changed from `Vec<Village>` to `PrefixResult` (breaking)
- `find_by_code_prefix` signature now includes `offset: usize` parameter (breaking)
- `DataInfo.source` and `DataInfo.decree` changed from `&'static str` to `String` (breaking)
- `data_info()` now reads from `db_meta` table instead of build-time env vars; returns `0`/`"unknown"` defaults for DBs built before v0.4.0
- `Pipeline::build_db()` now creates `db_meta` table with decree, source, build date, and village count
- Removed pipeline mode from `build.rs` (`WILAYAH_BUILD_PIPELINE=1`); builder now only runs via `cargo run --example build_db --features build-db`
- Moved builder dependencies (`regex`, `serde_json`, `sha2`) from `[build-dependencies]` to optional `[dependencies]` behind `build-db` feature
- Removed `rusqlite` from `[build-dependencies]`; lookup-only users now compile ~126 fewer packages
- Renamed `wilayah::pipeline` module to `wilayah::builder` (breaking)

## 0.3.0 - 2026-05-19

### Added

- `pipeline` module (feature-gated behind `build-db`) — full data build process as reusable library
- `examples/build_db.rs` — CLI wrapper for pipeline
- `examples/verify_legacy.rs` — standalone verification tool
- GitHub Release artifact distribution for pre-built database
- `build.rs` modes: download (default) and pipeline (`WILAYAH_BUILD_PIPELINE=1`)
- `build.rs` download fallback: fetches `locations.db` from GitHub Releases when not present locally

### Changed

- Build process: default `cargo build` downloads pre-built DB from GitHub Releases (instant builds)
- Documentation updated for new architecture and build workflow
- `cargo publish` now passes without writing outside `OUT_DIR`

### Fixed

- Removed build-time verification from pipeline (now in standalone tool)
- Fixed several build script issues to enable publishing

## 0.2.0 - 2026-05-18

### Added

- `find_by_code()` — direct lookup by administrative code
- `find_by_code_prefix()` — hierarchical lookup (kecamatan/kabupaten/province)
- `Display` impl for `Village` and `LookupResult` (CLI-friendly output)
- `/code` endpoint in HTTP server example (exact + prefix lookup)
- CI/CD pipeline (`.github/workflows/ci.yml`)
- `RELEASE.md` with release workflow documentation

### Fixed

- `polygon_centroid()` bug — was pushing entire `rings` array instead of iterating individual rings (produced all-zero coordinates)
- PDF annotation keyword stripping — expanded from 4 to 22+ keyword variants, cleaning ~1,100 village names that previously contained stale government annotations

### Changed

- Repository URL updated to `github.com/rafiyq/wilayah`
- `extract_village_name()` refactored from fragile `.split().next().unwrap_or()` chain to "find earliest keyword" approach

## [0.1.0] - 2025-05-16

### Added

- Initial release
- 82,689 villages from Kepmendagri No 300.2.2-2430 Tahun 2025
- SQLite embedded database with RTree spatial index and FTS5 full-text search
- `open()` — open the embedded database
- `find_nearest()` — nearest village lookup by GPS coordinates
- `find_by_name()` — full-text village name search (FTS5, BM25 ranked)
- `data_info()` — metadata about the embedded database (source, decree, count, build date)
- `version()` — crate version string
- `village_count()` — total number of villages in the database
- `Village` struct — serializable result type with code, name, hierarchy, coordinates
- HTTP server example (`cargo run --example serve`) with `/nearest` and `/search` endpoints
- Graceful build errors with actionable messages
- `WILAYAH_DATA_DIR` env var support for offline builds
- Pure Rust build pipeline (no Python or external tools required)
