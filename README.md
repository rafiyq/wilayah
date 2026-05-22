# wilayah

Location lookup for Indonesian villages by GPS coordinates or name.

Returns BMKG-compatible `adm4` administrative codes (e.g., `31.71.03.1001`)
for **83,758 villages** across Indonesia, based on the official Kemendagri
decree (Kepmendagri No 300.2.2-2138 Tahun 2025) with coordinates computed from
BIG (Badan Informasi Geospasial) polygon boundaries.

## Features

- **Nearest village search** by GPS with Haversine distance
- **Full-text search** (FTS5) across village name, district, city, province
- **Exact code lookup** and hierarchical prefix queries
- **Disambiguation helper** (`find_by_name_unique`) for unambiguous results
- **Single portable binary** with embedded ~27 MB SQLite database (RTree spatial index)

## Quick start (library)

```rust
use wilayah;

let conn = wilayah::open()?;

// Find nearest villages by GPS
let nearest = wilayah::find_nearest(&conn, -6.1647, 106.8453, 5)?;

// Search by name (FTS5, BM25 ranked)
let results = wilayah::find_by_name(&conn, "kemayoran", 10)?;

// Unambiguous lookup — returns Found, Ambiguous, or NotFound
let unique = wilayah::find_by_name_unique(&conn, "gambir")?;
println!("{unique}"); // "Gambir — Gambir, Kota Administrasi Jakarta Pusat, ... (31.71.05.1001)"

// Direct lookup by administrative code
let v = wilayah::find_by_code(&conn, "31.71.03.1001")?;

// List all villages in a kecamatan, kabupaten, or province (paginated)
let result = wilayah::find_by_code_prefix(&conn, "31.71.03", 100, 0)?;
println!("{} of {} villages", result.villages.len(), result.total);
```

## Quick start (HTTP server)

```bash
cargo run --release --example serve

curl "http://localhost:3000/nearest?lat=-6.1647&lon=106.8453"
curl "http://localhost:3000/search?q=Kemayoran"
curl "http://localhost:3000/code?q=31.71.03.1001"
curl "http://localhost:3000/code?prefix=31.71.03"
```

Alternatively, build and run the binary directly:

```bash
cargo build --release
./target/release/examples/serve
```

## API

### `GET /`

Server info and village count.

### `GET /nearest`

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `lat` | f64 | required | Latitude (-90..90) |
| `lon` | f64 | required | Longitude (-180..180) |
| `limit` | usize | 5 | Max results (1..20) |

### `GET /search`

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `q` | string | required | Search query |
| `limit` | usize | 5 | Max results (1..100) |

### `GET /code`

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `q` | string | optional | Exact administrative code (e.g., `31.71.03.1001`) |
| `prefix` | string | optional | Code prefix (e.g., `31.71.03` for all villages in a kecamatan) |
| `limit` | usize | 100 | Max results for prefix search (1..1000) |
| `offset` | usize | 0 | Number of results to skip (for pagination) |

Provide either `q` or `prefix`. Exact lookup returns `{"result": {...}}` (or `null`),
prefix returns `{"results": [...], "total": N, "has_more": bool}`.

## Data

| | |
|---|---|
| **Source** | Official Kemendagri PDF + BIG ArcGIS API |
| **Decree** | Kepmendagri No 300.2.2-2138 Tahun 2025 |
| **Villages** | 83,758 across all provinces (including Papua) |
| **Database** | SQLite with RTree and FTS5 (~27 MB embedded) |
| **License** | MIT |

Data pipeline (build-time only):
- **Kemendagri PDF** (57 MB, 4,428 pages) → village codes and names via `pdftotext` parser
- **BIG ArcGIS API** → village polygon geometries → centroid computation (84 batches, 83k+ features)
- **Merge** → match by administrative code; fallback to kecamatan centroid for new villages (pemekaran)
- **Build** → SQLite with indexes, RTree, FTS5, SHA-256 signature

## Building: download vs pipeline

By default, `cargo build` downloads a pre-built database from the GitHub Releases
artifact (no build-time dependencies needed). This gives instant builds.

To rebuild the database from scratch (e.g., for data updates or verification),
use the `build_db` example:

```bash
# Remove any cached database
rm -rf data/cache data/locations.db

# Run full pipeline (requires pdftotext, network access)
cargo run --example build_db --features build-db

# After it completes, you can build the library as usual
cargo build
```

Set `WILAYAH_REFRESH_BIG=1` to force re-fetch from BIG API:

```bash
WILAYAH_REFRESH_BIG=1 cargo run --example build_db --features build-db
```

The build script (`build.rs`) downloads a pre-built database from the latest
GitHub Release into `OUT_DIR`. If `data/locations.db` exists locally (from a
previous pipeline run), it copies that instead.

### Environment variables

| Variable | Effect |
|----------|--------|
| `WILAYAH_REFRESH_BIG=1` | Force re-fetch BIG data from ArcGIS API (pipeline only) |
| `WILAYAH_VERIFY_VERBOSE=1` | Print detailed verification in comparison tool |

## Architecture

### Components

- **Library** `src/lib.rs`: public API (`open`, `find_nearest`, `find_by_name`, etc.)
- **Database layer** `src/db.rs`: SQLite access (RTree, FTS5)
- **Pipeline** `src/pipeline.rs`: full data build process (PDF → BIG → DB)
- **Build script** `build.rs`: download-or-build shim (uses `#[path]` to reuse pipeline)
- **Examples**:
  - `serve` — axum HTTP API server (default)
  - `build_db` — CLI wrapper for `Pipeline` (requires `build-db` feature)
  - `verify_legacy` — compare official DB with legacy snapshot (no feature needed)

### Build flow

```text
cargo build → build.rs (download) → copy/Download DB → embed
cargo run --example build_db --features build-db → Pipeline.run() → build DB
cargo run --example verify_legacy → compare embedded DB vs data/cache/legacy_snapshot.json
```

The pipeline code lives in `src/pipeline.rs`, accessible as `wilayah::pipeline`
when the `build-db` feature is enabled. Both the `build_db` example and any
programmatic usage share the same `Pipeline` struct.

## Verification

The verification tool compares the current database with the legacy community-sourced
dataset (cahyadsn/wilayah) and prints a report:

```bash
cargo run --example verify_legacy
```

Output includes:
- New villages (present in official but not legacy)
- Missing villages (present in legacy but not official)
- Name differences
- Coordinate drift > 1km
- Hierarchy differences (district/city/province)

The verification code is **not part of the build**. The legacy snapshot is
generated automatically on first official pipeline run and saved to
`data/cache/legacy_snapshot.json`.

## Disambiguation

`find_by_name_unique()` returns a [`LookupResult`](https://docs.rs/wilayah/latest/wilayah/enum.LookupResult.html) with `Display` support for CLI-friendly output:

| Result | `println!("{result}")` output |
|--------|-------------------------------|
| `Found(v)` | `"Gambir — Gambir, Kota Administrasi Jakarta Pusat, ... (31.71.05.1001)"` |
| `Ambiguous(list)` | Numbered list of candidates + suggestion to refine query |
| `NotFound` | `"No matching village found"` |

Both [`Village`](https://docs.rs/wilayah/latest/wilayah/struct.Village.html) and `LookupResult` implement `Display` and `serde::Serialize`.

## License

MIT
