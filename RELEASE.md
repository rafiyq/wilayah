# Release workflow

## Prerequisites

- `pdftotext` installed (`apt install poppler-utils` or `brew install poppler`)
- Network access (fetches data on first build)

## Steps

1. **Update version**

   ```bash
   # Edit version in Cargo.toml
   $EDITOR Cargo.toml
   ```

2. **Rebuild database from scratch**

   ```bash
   rm -f data/locations.db data/cache/big_villages.json
   WILAYAH_REFRESH_BIG=1 cargo build --release
   ```

   This runs the full pipeline:
   - Downloads Kemendagri PDF (SHA-256 verified)
   - Fetches BIG polygon centroids from ArcGIS API (~84 batches)
   - Merges and validates data
   - Builds SQLite database with RTree + FTS5
   - Verifies against legacy snapshot (if available)

3. **Run tests**

   ```bash
   cargo test
   ```

4. **Commit and tag**

   ```bash
   git add -A
   git commit -m "v<version>: <summary>"
   git tag "v<version>"
   git push && git push --tags
   ```

5. **Publish to crates.io**

   ```bash
   cargo publish --dry-run
   cargo publish
   ```

6. **Create GitHub release**

   ```bash
   gh release create "v<version>" \
     --title "v<version>" \
     --notes "See changelog"
   ```

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

## Changelog template

```markdown
## v0.x.y (YYYY-MM-DD)

### Added
- ...

### Fixed
- ...

### Changed
- ...
```
