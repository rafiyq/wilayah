# wilayah

A location lookup library and API for Indonesian villages. Given GPS coordinates
or a village name, returns the BMKG-compatible adm4 administrative code.

## Context

This project supports `cuaca` (the BMKG weather indicator for Waybar) by solving
the problem of how users find their adm4 code. BMKG's API requires codes like
`31.71.03.1001`, but users know locations by name or GPS coordinates.

## Dataset

**Source:** `cahyadsn/wilayah` + `cahyadsn/wilayah_boundaries`
- 82,689 villages (Kepmendagri No 300.2.2-2430 Tahun 2025)
- BMKG-compatible adm4 codes (e.g. `31.71.03.1001`)
- Pre-computed centroids from BIG polygon boundaries
- Name hierarchy (desa, kecamatan, kotkab, provinsi)
- MIT license
- All 38 provinces including Papua regions

## API Endpoints (serve example)

```
GET /                                      Server info + village count
GET /nearest?lat=-6.16&lon=106.85&limit=5 Nearest villages
GET /search?q=Kemayoran&limit=10           Search villages
```

## Architecture

- **Language:** Rust
- **Crate:** `wilayah` — SQLite + RTree + FTS5 + Haversine (single library crate)
- **HTTP server:** `examples/serve.rs` — axum HTTP server wrapping the library
- **Database:** SQLite (embedded via `include_bytes!`, 20.7 MB)
- **Deployment:** `cargo run --release --example serve` → ~25 MB binary, zero runtime deps

## Relationship to cuaca

`cuaca` uses `wilayah` as a Rust library to resolve `--lat`/`--lon` or `--name`
flags into adm4 codes, then fetches weather from BMKG.

## Data pipeline

```bash
python3 scripts/build_db.py --kel-dir /tmp/wilayah_boundaries/db/kel
```

Pre-built `locations.db` is committed and embedded at compile time.

## Progress

- [x] Research
- [x] Bootstrap project structure
- [x] Data preprocessing pipeline
- [x] SQLite + RTree setup
- [x] API server
- [x] Integration with cuaca