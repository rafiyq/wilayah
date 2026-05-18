# wilayah

Location lookup for Indonesian villages by GPS coordinates or name.

Returns BMKG-compatible `adm4` administrative codes (e.g., `31.71.03.1001`)
for 83,758 villages across Indonesia, sourced from the official Kemendagri
decree (Kepmendagri No 300.2.2-2138 Tahun 2025) with village centroids
computed from BIG (Badan Informasi Geospasial) polygon boundaries.

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

// List all villages in a kecamatan, kabupaten, or province
let villages = wilayah::find_by_code_prefix(&conn, "31.71.03", 100)?;
```

## Quick start (HTTP server)

```bash
cargo run --release --example serve

curl "http://localhost:3000/nearest?lat=-6.1647&lon=106.8453"
curl "http://localhost:3000/search?q=Kemayoran"
curl "http://localhost:3000/code?q=31.71.03.1001"
curl "http://localhost:3000/code?prefix=31.71.03"
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

Provide either `q` or `prefix`. Exact lookup returns `{"result": {...}}` (or `null`),
prefix returns `{"results": [...]}`.

## Data

| | |
|---|---|
| **Administrative codes** | Kemendagri (Kepmendagri No 300.2.2-2138 Tahun 2025) |
| **Village coordinates** | BIG polygon boundaries (computed centroids) |
| **Villages** | 83,758 across 38 provinces (all Papua regions included) |
| **License** | MIT |

Data is sourced directly from official government sources:
- Administrative codes: Kemendagri PDF decree
- Village boundaries: BIG ArcGIS REST API (polygon geometries)

### Building

The first `cargo build` downloads the official Kemendagri PDF (~57 MB) and
BIG village boundary data (~83K records), then builds a ~27 MB SQLite database.
BIG data is cached locally, so subsequent builds complete in ~20 seconds.
Requires network access on first build only.

```bash
cargo build          # downloads data, builds DB, compiles (~20s after cache)
cargo build          # uses cached DB, instant
```

### Manually rebuilding the database

```bash
rm data/locations.db data/cache/big_villages.json -f
cargo build          # downloads fresh data and rebuilds
```

### Environment variables

| Variable | Description |
|----------|-------------|
| `WILAYAH_REFRESH_BIG=1` | Force re-fetch BIG village data from ArcGIS API |
| `WILAYAH_VERIFY_VERBOSE=1` | Print detailed verification report comparing official vs legacy data |

## Architecture

- **Crate:** `wilayah` — SQLite + RTree + FTS5 + Haversine (single library crate)
- **HTTP server:** `examples/serve.rs` — axum HTTP server wrapping the library
- **Database:** SQLite (embedded via `include_bytes!`, ~27 MB)
- **Deployment:** `cargo run --release --example serve` → ~25 MB binary, zero runtime deps

```text
wilayah (library)
├── open() → embedded SQLite connection
├── find_nearest() → RTree spatial index + Haversine distance
├── find_by_name() → FTS5 full-text search (BM25 ranked)
├── find_by_name_unique() → disambiguation helper (Display-friendly)
├── find_by_code() → direct lookup by administrative code
├── find_by_code_prefix() → hierarchical lookup (kecamatan/kabupaten/province)
├── data_info() → source, decree, version, build date
└── village_count() → total villages in database
```

Data pipeline (build-time only)
├── PDF extraction → pdftotext → state machine parser
├── BIG API → ArcGIS REST → polygon centroid computation
├── Merge → PDF codes + BIG coordinates
└── Verification → cross-reference with legacy data
```

## Disambiguation

`find_by_name_unique()` returns a [`LookupResult`](https://docs.rs/wilayah/latest/wilayah/enum.LookupResult.html) with `Display` support for CLI-friendly output:

| Result | `println!("{result}")` output |
|--------|-------------------------------|
| `Found(v)` | `"Gambir — Gambir, Kota Administrasi Jakarta Pusat, ... (31.71.05.1001)"` |
| `Ambiguous(list)` | Numbered list of candidates + suggestion to refine query |
| `NotFound` | `"No matching village found"` |

Both [`Village`](https://docs.rs/wilayah/latest/wilayah/struct.Village.html) and `LookupResult` implement `Display` and `serde::Serialize`.

## Verification

The official data pipeline includes automatic verification against the legacy
community-sourced data (cahyadsn/wilayah). On first official build, a snapshot
of the legacy data is saved. Subsequent builds compare official data against
this snapshot and print a summary report:

- New villages: present in official but not legacy (2025 pemekaran)
- Missing villages: present in legacy but not official (potential data loss)
- Name differences: same code, different village name
- Coordinate drift: BIG vs legacy centroid distance > 1km
- Hierarchy differences: different district/city/province assignment

## License

MIT
