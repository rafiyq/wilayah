# wilayah Cloudflare Worker

A Cloudflare Worker that serves the wilayah Indonesian village lookup API, backed by Cloudflare D1 (managed SQLite).

## Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /` | API info and village count |
| `GET /nearest?lat=&lon=&limit=` | Find nearest villages by coordinates |
| `GET /search?q=&limit=` | Search villages by name (LIKE) |
| `GET /code?q=` | Exact code lookup |
| `GET /code?prefix=&limit=&offset=` | Code prefix lookup with pagination |
| `GET /locate?lat=&lon=` | Reverse-geocode to administrative hierarchy |

All responses are JSON with CORS headers.

## Setup

### Prerequisites

- [Rust](https://rustup.rs/) with `wasm32-unknown-unknown` target
- [Node.js](https://nodejs.org/) (for wrangler)
- [Wrangler](https://developers.cloudflare.com/workers/wrangler/): `npm install -g wrangler`
- A Cloudflare account with D1 access

### 1. Add wasm32 target

```bash
rustup target add wasm32-unknown-unknown
```

### 2. Create a D1 database

```bash
wrangler d1 create wilayah-locations
```

Copy the `database_id` from the output and update `wrangler.toml`:

```toml
[[d1_databases]]
binding = "DB"
database_name = "wilayah-locations"
database_id = "YOUR_DATABASE_ID_HERE"
```

### 3. Import data

First, ensure the wilayah database exists at `../../data/locations.db`. If not:

```bash
cd ../..  # back to wilayah repo root
cargo run --example build_db --features build-db
cd examples/cloudflare-worker
```

Then run the import script:

```bash
./import_db.sh
```

This creates the schema, exports data from the local SQLite database, and imports it into D1 in batches.

### 4. Run locally

```bash
npx wrangler dev
```

### 5. Deploy

```bash
npx wrangler deploy
```

## Differences from the axum server example

| Feature | axum `serve.rs` | Cloudflare Worker |
|---------|-----------------|-------------------|
| Database | Embedded in-memory SQLite | Cloudflare D1 (remote) |
| Spatial index | RTree + Haversine UDF | Bounding box + Rust-side Haversine |
| Full-text search | FTS5 (BM25 ranked) | LIKE queries |
| Runtime | Tokio async runtime | Workers runtime (WASM) |
| Scaling | Single process | Edge network, global |

The Cloudflare Worker trades some query precision (no FTS5 ranking, no RTree index) for global edge deployment with zero infrastructure management. For most use cases, the bounding-box + Haversine approach gives equivalent results to the RTree approach for the `/nearest` and `/locate` endpoints.
