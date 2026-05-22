# TODO

## Own the Data Pipeline (Path B)
- [x] Phase 1: Pipeline scaffolding — `build.rs` inline modules + cache utilities
- [x] Phase 2: PDF extraction — download Kemendagri PDF, verify SHA-256, extract text via `pdftotext`
- [x] Phase 3: Village parser — state machine: PDF text → structured village records
- [x] Phase 4: BIG API — query ArcGIS REST, compute centroids from polygon geometry
- [x] Phase 5: Merge & validate — join PDF + BIG data, discrepancy report, 3-way cross-reference
- [x] Phase 6: Database builder — SQLite with RTree + FTS5 + provenance table
- [x] Phase 7: Cleanup — remove `wilayah.sql`, update README, remove old pipeline code
- [x] Phase 8: Data verification — hybrid comparison (DB-to-DB or snapshot), discrepancy report
- [x] Phase 9: Final cleanup — remove legacy pipeline code, update dependencies

## Library
- [x] Add `wilayah::find_by_code(kode)` for direct adm4 lookup
- [x] Add `wilayah::find_by_code_prefix(prefix, limit)` for hierarchical lookup
- [x] Improve ambiguous name UX for CLI consumers — added `Display` for `Village` and `LookupResult`

## DevOps
- [x] CI/CD pipeline (test + build on push) — `.github/workflows/ci.yml`
- [x] Document release workflow — `RELEASE.md`
- [x] Fix `polygon_centroid` bug — was pushing entire `rings` array as one item instead of iterating individual rings

## Data quality
- [x] Expand PDF annotation keyword stripping — added `Koreksi`, `Penggabungan`, `Pembentukan`, `Penetapan`, `Perubahan`, `Peningkatan`, `Pemecahan`, `Nagari hasil`, case variants, and space-prefixed `hasil`/`Hasil`

## Build System Restructure (Completed)

- [x] Split pipeline from `build.rs` into `src/pipeline.rs`
- [x] Add `build-db` feature to gate pipeline dependencies
- [x] Rewrite `build.rs` to support download mode (default) and pipeline mode (`WILAYAH_BUILD_PIPELINE=1`)
- [x] Add `examples/build_db.rs` CLI wrapper for pipeline
- [x] Add `examples/verify_legacy.rs` standalone verification tool
- [x] Distribute pre-built DB via GitHub Release (download in build.rs)
- [x] Update CI to use `build_db` example for fresh build
- [x] Update `RELEASE.md` and `README.md` with new workflow

## v0.4.0 (Completed)

- [x] Pagination for `find_by_code_prefix` — `PrefixResult` with `total`, `has_more`, `offset`
- [x] `db_meta` table in pipeline-built databases
- [x] `data_info_from_conn()` public function
- [x] `PartialEq` on `Village` and `DataInfo`
- [x] `DataInfo` fields changed from `&'static str` to `String`
- [x] Pipeline unit tests
- [x] Update README and RELEASE.md for v0.4.0 API

## Future

- [ ] Build-dependency optimization — split pipeline into separate workspace crate (deferred from v0.4.0)
- [ ] `locate` function — reverse geocode administrative hierarchy from coordinates
