# wilayah

Location lookup for Indonesian villages by GPS coordinates or name.

Returns BMKG-compatible `adm4` administrative codes (e.g., `31.71.03.1001`)
for 82,689 villages across Indonesia.

## Quick start (library)

```rust
use wilayah;

let conn = wilayah::open()?;
let nearest = wilayah::find_nearest(&conn, -6.1647, 106.8453, 5)?;
let results = wilayah::find_by_name(&conn, "kemayoran", 10)?;
```

## Quick start (HTTP server)

```bash
cargo run --release --example serve

curl "http://localhost:3000/nearest?lat=-6.1647&lon=106.8453"
curl "http://localhost:3000/search?q=Kemayoran"
```

## Building

The first `cargo build` downloads ~300 MB of raw data from GitHub and builds a
20 MB SQLite database. This is cached locally, so subsequent builds are fast.
Requires network access on first build only.

```bash
cargo build          # downloads data, builds DB, compiles
cargo build          # uses cached DB, instant
```

## Data

Sourced from [cahyadsn/wilayah](https://github.com/cahyadsn/wilayah) and
[cahyadsn/wilayah_boundaries](https://github.com/cahyadsn/wilayah_boundaries),
based on Kepmendagri No 300.2.2-2430 Tahun 2025.

**82,689 villages** with pre-computed centroids from BIG polygon boundaries.

### Manually rebuilding the database

```bash
rm data/locations.db data/raw -rf
cargo build          # downloads fresh data and rebuilds
```

## Relationship to cuaca

[cuaca](https://github.com/your-org/cuaca) is a BMKG weather indicator for Waybar.
It uses `wilayah` as a library to resolve `--lat`/`--lon` or `--name` flags
into adm4 codes for BMKG's weather API.
