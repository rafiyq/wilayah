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
   - Build the database from the pipeline
   - Run tests
   - Publish the crate to crates.io
   - Create a GitHub Release with the title `v<version>` and the changelog from `CHANGELOG.md`
   - Upload the `data/locations.db` artifact to the release

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
   cargo run --example build_db --features build-db
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
   gh release upload "v<version>" data/locations.db
   ```

   The pre-built database is not committed to the repository; it is distributed via GitHub Releases.

## Post-release

The build script (`build.rs`) will automatically download the database from the latest GitHub Release on `cargo build` for downstream users, eliminating the need to ship the ~27 MB DB in the source tree.

## Versioning

Follow semantic versioning. The public API is:

- `wilayah::open()` — stable
- `wilayah::find_nearest()` — stable
- `wilayah::find_by_name()` — stable
- `wilayah::find_by_name_unique()` — stable
- `wilayah::find_by_code()` — stable
- `wilayah::village_count()` — stable
- `wilayah::data_info()` — stable
- `wilayah::version()` — stable

Breaking changes (major version bump):

- Schema changes to the embedded database
- Removal or signature change of public functions
- Data source changes that significantly alter results
