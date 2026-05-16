# wilayah

Location lookup for Indonesian villages by GPS coordinates or name.

Returns BMKG-compatible `adm4` administrative codes (e.g., `31.71.03.1001`)
for 82,689 villages across Indonesia, based on official Kemendagri administrative
codes with pre-computed village centroids from BIG (Badan Informasi Geospasial)
polygon boundaries.

## Quick start (library)

```rust
use wilayah;

let conn = wilayah::open()?;
let nearest = wilayah::find_nearest(&conn, -6.1647, 106.8453, 5)?;
let results = wilayah::find_by_name(&conn, "kemayoran", 10)?;
let unique  = wilayah::find_by_name_unique(&conn, "gambir")?;
```

## Quick start (HTTP server)

```bash
cargo run --release --example serve

curl "http://localhost:3000/nearest?lat=-6.1647&lon=106.8453"
curl "http://localhost:3000/search?q=Kemayoran"
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

## Data

| | |
|---|---|
| **Administrative codes** | Kemendagri (Kepmendagri No 300.2.2-2138 Tahun 2025) |
| **Village coordinates** | BIG polygon boundaries (pre-computed centroids) |
| **Villages** | 82,689 across 38 provinces (all Papua regions included) |
| **License** | MIT |

Data is sourced from upstream community repos
([cahyadsn/wilayah](https://github.com/cahyadsn/wilayah) and
[cahyadsn/wilayah_boundaries](https://github.com/cahyadsn/wilayah_boundaries))
which faithfully transcribe official Kemendagri and BIG sources.

### Building

The first `cargo build` downloads ~300 MB of raw data from GitHub and builds a
20 MB SQLite database. This is cached locally, so subsequent builds are fast.
Requires network access on first build only.

```bash
cargo build          # downloads data, builds DB, compiles
cargo build          # uses cached DB, instant
```

For offline builds, set `WILAYAH_DATA_DIR` to a directory containing
`wilayah.sql` and `kel/*.sql` files.

### Manually rebuilding the database

```bash
rm data/locations.db data/raw -rf
cargo build          # downloads fresh data and rebuilds
```

## Architecture

- **Crate:** `wilayah` — SQLite + RTree + FTS5 + Haversine (single library crate)
- **HTTP server:** `examples/serve.rs` — axum HTTP server wrapping the library
- **Database:** SQLite (embedded via `include_bytes!`, ~21 MB)
- **Deployment:** `cargo run --release --example serve` → ~25 MB binary, zero runtime deps

```
wilayah (library)
├── open()              → embedded SQLite connection
├── find_nearest()      → RTree spatial index + Haversine distance
├── find_by_name()      → FTS5 full-text search (BM25 ranked)
├── find_by_name_unique() → disambiguation helper
├── data_info()         → source, decree, version, build date
└── Village struct      → code, name, hierarchy, coordinates, dist_km
```

## Relationship to cuaca

[cuaca](https://github.com/your-org/cuaca) is a BMKG weather indicator for Waybar.
It uses `wilayah` as a library to resolve `--lat`/`--lon` or `--name` flags
into adm4 codes for BMKG's weather API.
