# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

### Changed

- Poly DB path derivation refactored: `with_extension("poly.db")` replaced with `parent().join(constant)` — always produces `locations-poly.db` regardless of main DB filename; added `DEFAULT_DB_FILENAME`, `DEFAULT_POLY_DB_FILENAME`, `DEFAULT_OUTPUT_DIR` public constants
- `Village::new()` constructor added — allows external construction despite `#[non_exhaustive]`; `with_dist_km()` builder-style setter for nearest-neighbor results
- `Location::new()` constructor added — same rationale, enables external construction despite `#[non_exhaustive]`

## 0.6.0 - 2026-07-01

### Added

- serde feature flag — enables Serialize on all public types independently of db; db implies serde automatically
- build-db test step in CI workflow (59 builder unit tests previously never ran in CI)
- test_deserialize_vertices_aligned test
- builder/util.rs — shared utilities (fetch_url_with_retry, fetch_with_retry, hash_sha256, hash_file) extracted from pdf.rs and db_create.rs
- tests/builder.rs — PipelineError unit tests with build-db feature gate (moved from tests/types.rs)
- `#[non_exhaustive]` on Village, Location, DataInfo, LookupResult, PrefixResult, PipResult, and db::Error for forward-compatibility
- `collect_rows()` helper in db/query.rs to reduce repeated row-collection boilerplate
- `PipelineError::from_error()` private constructor to deduplicate From impls
- `VILLAGE_COLS`/`VILLAGE_COLS_L` SQL column list constants with doc mapping Indonesian→English field names
- `lock_conn()`/`lock_poly()` Mutex helpers on Database
- `set_write_pragmas()` helper in db_create.rs for PRAGMA initialization
- `locate_contained()` extracted into db/polygon.rs for polygon containment logic
- `Default` impl on `DataInfo` (used by `cached_data_info()` fallback)
- `tests/db_types.rs` — Database and Error integration tests (split from tests/types.rs)
- `DistrictRecord` struct with kel_count, desa_count, district_note; `parsed_districts.json` sidecar output
- `ProvinceRecord` struct with ibukota, luas, penduduk, island_count; `parsed_provinces.json` sidecar output
- `CityRecord` struct with ibukota, male_pop, female_pop, total_pop, luas, penduduk, island_count; `parsed_cities.json` sidecar output
- `IslandSummary` struct (code, name, province, island_count); `parsed_island_summaries.json` sidecar output
- `IslandRecord` struct (code, name, kabupaten_code, latitude, longitude, area_km2, status, keterangan); `parsed_islands.json` sidecar output
- `VillageRecord` enriched with kel_count, desa_count, keterangan fields
- `extract_district_name()` with column-gap splitting and code-as-name detection
- `strip_district_note()` and `extract_suffix_note()` with word boundary checks
- `parse_province_rest()` with `find_first_large_gap()` for ibukota extraction from Section C province headers
- `parse_section_db()`, `parse_section_dc()`, `parse_island_tail()`, `parse_island_fields()`, `coord_regex()`, `extract_islands()` — island data parsing from PDF sections D.b + D.c
- Population data parsing from Section E (male, female, total per kabupaten/kota)
- 40 new unit tests (14 island tests + district/note/keyword tests)
- `required-features = ["db"]` on serve example (prevented compilation with --no-default-features)

### Changed

- builder/geometry.rs renamed to builder/spatial.rs to avoid confusion with src/geometry.rs
- MergedVillage fields renamed to English (code, name, district, city, province); SQL column names remain Indonesian
- save_parsed_villages uses #[serde(skip_serializing_if)] instead of manual JSON construction
- parse_rings_from_json removed from big_api.rs in favor of spatial::extract_rings
- Cross-module imports fixed: crate::types::bbox/serialize_vertices/point_in_polygon → crate::geometry::*
- BIG API cache filenames now distinguish polygon vs non-polygon builds: big_villages_with_polygons.json vs big_villages.json
- WILAYAH_REFRESH_BIG=1 env var is now read by build_db example (was documented but never implemented)
- village_count() uses try_into().expect() instead of count as u32 to avoid silent truncation
- All .lock().unwrap() on Mutex replaced with .lock().unwrap_or_else(|e| e.into_inner()) to recover from poisoning
- json_str/json_str_or changed from pub(crate) to private (only used within big_api.rs)
- Version test uses env!("CARGO_PKG_VERSION") instead of hardcoded "0.5.1"
- data_info() doc comment now notes it requires the db feature flag
- Test regex patterns compiled once via OnceLock helpers (was 22 duplicate compilations)
- **BREAKING**: `parent_ring_id` column removed from village_polygons schema (was always NULL; affects existing polygon DBs at 0.x)
- `bbox` and `serialize_vertices` feature-gated behind `cfg(any(test, feature = "build-db"))`; `deserialize_vertices` behind `cfg(any(test, feature = "db"))`
- `deserialize_vertices` now returns `Result<Vec<(f64, f64)>, String>` instead of panicking on misaligned blobs
- src/db.rs split into src/db/ module directory (mod.rs, query.rs, polygon.rs, meta.rs)
- 5 unit tests moved from tests/locate.rs to inline #[cfg(test)] in src/types.rs
- `compute_centroid`, `extract_rings`, `polygon_centroid` moved from spatial.rs to big_api.rs (private)
- `data_info()` / `cached_data_info()` no longer panics — returns defaults if DB open fails
- geometry integration tests moved inline into src/geometry.rs (tests/geometry.rs removed)
- SQL column references in db/query.rs use `VILLAGE_COLS`/`VILLAGE_COLS_L` constants instead of hardcoded strings
- `village_from_row()` takes `dist_col: bool` parameter instead of separate row-mapping functions
- `hash_file()` uses streaming SHA-256 (8 KiB chunks) instead of reading entire file into memory
- Redundant `district_code.clone()` in parse.rs replaced with direct assignment
- `compute_sha256` trivial wrapper removed — call sites use `util::hash_file()` directly
- `OfficialVillage`/`LegacyVillage` in verify_legacy.rs deduplicated into single `VillageEntry` struct
- Three `From` impls for `PipelineError` now use shared `from_error()` helper
- `tests/types.rs` split: pure type tests remain, Database/Error tests moved to `tests/db_types.rs`
- Poly schema duplication noted in tests/common/mod.rs with cross-reference to db_create.rs
- Response type duplication noted in serve.rs and Cloudflare Worker with cross-references
- Pipeline resummability limitation documented in Pipeline::run() doc comment
- `PipelineOutput` gained 5 new sidecar JSON path fields (parsed_provinces, parsed_cities, parsed_districts, parsed_island_summaries, parsed_islands)
- `Pipeline::run()` now extracts provinces, cities, and island data from PDF sections A, B, C headers, D.b, D.c, and E
- CI workflow refactored to matrix strategy testing all feature combinations (no-default-features, raw-sqlite, build-db)
- Release workflow clippy changed from --all-features to explicit per-feature-combo checks
- Release workflow now uploads all 8 sidecar JSON files (previously only 2)

### Fixed

- P1+P2: 177 truncated district names from column-gap misparse + missing district_note field
- P3: find_note_boundary false positives from partial keyword matches (added word boundary check)
- P4: PMD keyword not triggering note boundary (moved to self-validating keywords)
- P5: Missing note keywords for OCR typos (Semula, Berdasarkan, PP variants)
- P6: 322 dirty village names from column-gap splitting omission in extract_village_name
- P7+P8: Trailing periods on non-abbreviation names + all-lowercase names not capitalized
- P9: Word boundary checks in strip_district_note/extract_suffix_note prevent false splits
- CI cache key referenced stale src/builder.rs (now src/builder/mod.rs)
- RELEASE.md listed wilayah::open() (should be wilayah::Database::open()) and private data_info_from_conn() (removed)
- README.md referenced src/builder.rs (should be src/builder/)
- deserialize_vertices silently ignored misaligned blobs (now returns Err instead of debug_assert)
- Doc list indentation warning in lib.rs

## 0.5.0 - 2026-06-08

### Added

- `ParseOutputDetail` enum — controls detail level when saving parsed village records (`Minimal`, `WithRawName`, `Full`)
- `Pipeline::save_parsed_villages(detail)` — opt-in builder method to save PDF parser output as JSON
- `PipelineOutput::parsed_villages_path` — path to the saved parsed villages JSON
- `--include-polygons` flag on `build_db` example — enables polygon database output
- `--save-parsed-villages[=minimal|raw|full]` flag on `build_db` example — saves PDF parser output
- Release assets: `locations-poly.db`, `parsed_villages.json`, `big_villages.json` in addition to `locations.db`
- Reference-pattern validation for note keyword matching — keywords like "UU" and "ND" only trigger note stripping when followed by reference-like text (numbers, "No.", "nama", etc.), preventing false positives on village names containing these abbreviations
- Two-tier note keyword system: self-validating keywords (always indicate a note, e.g., "Semula", "Menjadi") and reference-validated keywords (require confirmation, e.g., "UU", "Perda")
- `extract_district_name()` and `skip_code_prefix()` — proper kecamatan name extraction that strips trailing " - VILLAGE_COUNT" and handles digit-starting district names
- `strip_trailing_separators()` — iterative cleanup of trailing dashes, commas, and spaces
- `has_reference_indicator()` and `find_note_boundary()` — core note detection logic with reference validation
- 26 new unit tests for parser accuracy (38 total, up from 12)

### Changed

- Note keyword list expanded from 13 to 34 keywords — added "Semula", "Qanun", "Amar", "UU", "ND", "Perda", "Perbup", "Kepbup", "PMD", "Surat", "Srt", "Penataan", "Pengkatan", "Penghapusan", "Berubah", "Lampiran", "Letak", "Ds.", "Afd.", "wil. Kec", "wil Kec", "Hal Hasil"
- Removed generic "Hasil" keyword (too broad — caused false positives on legitimate names); replaced with specific "Nagari hasil" and "Hal Hasil"
- Village name word truncation limit raised from 4 to 5 words — preserves Minangkabau names like "Tanah Sirah Piai Nan XX"
- `extract_village_name()` now lowercases the raw text once instead of 13 times per village
- Kecamatan regex changed from `[A-Z]` to `[A-Z0-9]` — allows district names starting with digits (e.g., "2 x 11 Anam Lingkuang")
- District name extraction rewritten — `rfind` + `find(alphabetic)` replaced with `extract_district_name()` + `skip_code_prefix()`, eliminating trailing " -" garbage in all ~7,000+ kecamatan names
- Village code regex changed from `^...\\s` to `^...(?:\\s|$)` — matches codes at end-of-line

- `Database` struct — wraps an internal SQLite connection, hiding `rusqlite` from the public API
- `wilayah::Error` — custom error type that wraps `rusqlite::Error` without exposing it
- `wilayah::Result<T>` — type alias for `std::result::Result<T, Error>`
- `raw-sqlite` feature flag — exposes `Database::conn()` and `Database::conn_guard()` for direct `rusqlite::Connection` access
- `types` module — always available, contains shared types independent of `rusqlite`
- `Serialize` derive on `PrefixResult`, `DataInfo`, `LookupResult`
- `Serialize` impl on `Error` (serializes as string)
- `Database` is now `Send + Sync` — internal `Connection` wrapped in `Mutex`
- `Database::conn_guard()` — safe accessor for the underlying `MutexGuard<Connection>` (`raw-sqlite` feature)
- `PipResult` enum — result of a point-in-polygon test (`Inside`, `Outside`, `OnBoundary`)
- `point_in_ring()` — ray-casting point-in-polygon for a single ring with on-boundary detection
- `point_in_polygon()` — point-in-polygon with optional interior rings (holes)
- `serialize_vertices()` / `deserialize_vertices()` — compact binary BLOB format for polygon vertices (16 bytes/point, little-endian f64 pairs)
- `RingClassification` enum — `SeparateRings` (default, treats all rings as exterior) and `ClassifyHoles` (spatial containment-based hole detection)
- `Database::open_with_polygons()` — opens main DB + separate polygon DB for containment-based `locate()`
- `Database::has_polygons()` — returns whether a polygon database is loaded
- `Pipeline::include_polygons()` — builder method to enable polygon database output
- `Pipeline::ring_classification()` — builder method to set ring classification mode
- `build_poly_db()` — builds `locations-poly.db` with village boundary geometry, bounding box indexes, and compact BLOB vertex storage
- `classify_rings()` — classifies multi-ring features into exterior/interior via bounding box pre-filter + PIP
- `extract_rings()` — extracts `[lat, lon]` ring arrays from BIG ArcGIS API geometry JSON
- `LocateMethod::Contained` is now functional — `locate()` returns it when the query point falls inside a village boundary polygon
- Re-exports: `PipResult`, `point_in_ring`, `point_in_polygon`, `serialize_vertices`, `deserialize_vertices`, `RingClassification`
- `serve.rs` example now accepts `--poly <path>` flag to load a polygon database
- Cloudflare Worker example with D1 backend (`examples/cloudflare-worker/`)
- `deploy-worker.yml` GitHub Actions workflow for Worker deployment
- `.gitignore` for cloudflare-worker directory and `.dev.vars`
- `PUT /update` and `PUT /update/meta` endpoints in Cloudflare Worker (auth-gated via `ADMIN_TOKEN` secret)
- CORS preflight (`OPTIONS`) handler in Cloudflare Worker
- `GET /locate` endpoint documented in README
- **BREAKING**: All query functions now take `&self` on `Database` instead of `&rusqlite::Connection` as the first parameter
- **BREAKING**: All query functions now return `wilayah::Result<T>` instead of `rusqlite::Result<T>`
- **BREAKING**: `open()` renamed to `Database::open()` and returns `Result<Database>` instead of `rusqlite::Result<Connection>`
- **BREAKING**: `village_count()` now returns `Result<u32>` instead of `Result<i64>` (consistent with `DataInfo.village_count`)
- **BREAKING**: `Database::conn()` replaced by `Database::conn_guard()` which returns `MutexGuard<Connection>` (derefs to `&Connection`)
- **BREAKING**: `location_from_village()` now takes `method: LocateMethod` as third parameter (was hardcoded `Nearest`)
- `Pipeline::run()` returns `PipelineOutput` with new `poly_db_path: Option<PathBuf>` field
- `Pipeline::fetch_big_data()` signature updated to accept `include_polygons: bool`; preserves ring geometry in cache when enabled
- `LocateMethod::Contained` is now functional (was previously a placeholder variant)
- `Database::locate()` dispatches to polygon containment when a polygon DB is loaded, with automatic fallback to nearest-centroid
- `data_info()` free function now internally uses `Database::open()` instead of `Connection::open_in_memory()` directly
- `Database::data_info()` method added as the preferred way to get metadata
- axum `serve.rs` example simplified: `Arc<Mutex<Database>>` → `Arc<Database>` (no more lock contention)
- CI workflow updated to Node 24-compatible action versions (`actions/checkout@v6`, `actions/cache@v5`)
- CI `clippy` now uses `--features raw-sqlite`
- CI `- run:` steps fixed to proper YAML indentation (were at 4 spaces instead of 6)
- Integration tests gated behind `raw-sqlite` feature (they use `Database::conn_guard()`)

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
