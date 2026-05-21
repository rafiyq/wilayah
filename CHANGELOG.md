# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- `find_by_code_prefix` now supports pagination with `offset` parameter and returns `PrefixResult` containing `total` and `has_more` (breaking change)
- Unit tests for core pipeline functions (`parse_section_header`, `extract_village_name`, `polygon_centroid`, `compute_centroid`, `merge_villages`, `parse_villages`, `build_db`)

### Changed

- `find_by_code_prefix` return type changed from `Vec<Village>` to `PrefixResult`
- `find_by_code_prefix` signature now includes `offset: usize` parameter

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
