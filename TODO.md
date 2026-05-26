# TODO

## v0.4.0 (Completed)

- [x] Pagination for `find_by_code_prefix` — `PrefixResult` with `total`, `has_more`, `offset`
- [x] `db_meta` table in builder-built databases
- [x] `data_info_from_conn()` public function
- [x] `PartialEq` on `Village` and `DataInfo`
- [x] `DataInfo` fields changed from `&'static str` to `String`
- [x] Builder unit tests
- [x] Rename `pipeline` module to `builder`
- [x] Remove `WILAYAH_BUILD_PIPELINE=1` from `build.rs`
- [x] Move builder deps from `[build-dependencies]` to optional `[dependencies]`
- [x] Update `release.yml` with `--features build-db`, clippy, changelog extraction
- [x] Update README and RELEASE.md for v0.4.0 API

## Future

- [ ] Builder workspace crate split — full separation into `wilayah-builder` (optional deps move was done in v0.4.0, workspace split deferred)
- [ ] `locate` function — reverse geocode administrative hierarchy from coordinates
