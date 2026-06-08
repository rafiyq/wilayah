# Release workflow

## Prerequisites

- `pdftotext` installed (`apt install poppler-utils` or `brew install poppler`)
- Network access
- (Optional) `gh` CLI installed and authenticated for manual releases
- (Automated releases) `CARGO_REGISTRY_TOKEN` secret configured in GitHub repo settings for crates.io publish

## Automated Release (Recommended)

The repository includes a GitHub Actions workflow that automates the entire release process.

### Steps

1. **Update version** in `Cargo.toml`.
2. **Commit and push** the version bump.
3. **Create and push a tag** matching `v*` pattern:

   ```bash
   git tag "v<version>"
   git push origin "v<version>"
   ```

4. The `release` workflow will automatically:
   - Build the database from the pipeline (with polygons + parsed villages)
   - Run tests
   - Publish the crate to crates.io
   - Create a GitHub Release with the title `v<version>` and the changelog from `CHANGELOG.md`
   - Upload the following release assets:
     - `locations.db` — main SQLite database (~27 MB)
     - `locations-poly.db` — polygon boundary database (for containment-based locate)
     - `parsed_villages.json` — PDF parser output with raw names (for auditing)
     - `big_villages.json` — BIG ArcGIS API cache (avoids re-fetching 84+ batches)

### Required secrets

Add `CARGO_REGISTRY_TOKEN` as a repository secret (Settings > Secrets and variables > Actions) with your crates.io API token.

### Notes

- The workflow file is `.github/workflows/release.yml`.
- The database is built from scratch using `cargo run --example build_db --features build-db` with `WILAYAH_REFRESH_BIG=1`.
- If any step fails, the workflow will stop and notify you.

## Manual Release (Alternative)

If you prefer to release manually, follow these steps:

1. **Update version** in `Cargo.toml`.
2. **Rebuild database from scratch**:

```bash
rm -rf data/cache data/locations.db
cargo run --example build_db --features build-db -- --include-polygons --save-parsed-villages raw
```

   This runs the full pipeline:
   - Downloads Kemendagri PDF (~57 MB)
   - Extracts text (~4,428 pages)
   - Fetches BIG polygon centroids from ArcGIS API (~84 batches, 83k+ villages)
   - Merges and validates data (pemekaran detection, kecamatan centroid fallback)
   - Builds SQLite database with RTree + FTS5
   - Prints SHA-256, village count

3. **Run tests**:

   ```bash
   cargo test
   ```

4. **Commit and tag**:

   ```bash
   git add -A
   git commit -m "v<version>: <summary>"
   git tag "v<version>"
   git push && git push --tags
   ```

5. **Publish to crates.io**:

   ```bash
   cargo publish --dry-run
   cargo publish
   ```

6. **Create GitHub release with database artifact**:

```bash
gh release create "v<version>" \
  --title "v<version>" \
  --notes "See CHANGELOG.md for details."
gh release upload "v<version>" \
  data/locations.db \
  data/locations-poly.db \
  data/cache/parsed_villages.json \
  data/cache/big_villages.json
```

The release assets are:
- `locations.db` — main SQLite database (downloaded by `build.rs` for downstream users)
- `locations-poly.db` — polygon boundary database (opt-in, for `LocateMethod::Contained`)
- `parsed_villages.json` — PDF parser output with raw village names (for auditing)
- `big_villages.json` — BIG ArcGIS API cache (avoids re-fetching 84+ API batches)

## Post-release

The build script (`build.rs`) will automatically download the database from the latest GitHub Release on `cargo build` for downstream users, eliminating the need to ship the ~27 MB DB in the source tree.

## Versioning

Follow semantic versioning. The public API is:

- `wilayah::open()` — stable
- `wilayah::find_nearest()` — stable
- `wilayah::find_by_name()` — stable
- `wilayah::find_by_name_unique()` — stable
- `wilayah::find_by_code()` — stable
- `wilayah::find_by_code_prefix()` — stable (pagination API since 0.4.0)
- `wilayah::PrefixResult` — stable (since 0.4.0)
- `wilayah::locate()` — stable (since 0.4.1)
- `wilayah::Location` — stable (since 0.4.1)
- `wilayah::AdminLevel` — stable (since 0.4.1)
- `wilayah::LocateMethod` — stable (since 0.4.1)
- `wilayah::Village` — stable
- `wilayah::LookupResult` — stable
- `wilayah::DataInfo` — stable (fields changed to `String` in 0.4.0, breaking)
- `wilayah::data_info()` — stable
- `wilayah::data_info_from_conn()` — stable (since 0.4.0)
- `wilayah::village_count()` — stable
- `wilayah::version()` — stable

Breaking changes (major version bump):

- Schema changes to the embedded database
- Removal or signature change of public functions
- Public struct field type changes (e.g., `DataInfo` `&'static str` → `String`)
- Data source changes that significantly alter results
