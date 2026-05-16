use rusqlite::{functions::FunctionFlags, Connection, Result};

const DB_BYTES: &[u8] = include_bytes!(env!("LOCATION_DB_PATH"));

const EARTH_RADIUS_KM: f64 = 6371.0;

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    EARTH_RADIUS_KM * 2.0 * a.sqrt().asin()
}

/// Open the embedded database connection and register the Haversine UDF.
pub fn open_embedded() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA journal_mode = OFF")?;
    conn.deserialize_bytes("main", DB_BYTES)?;

    conn.create_scalar_function(
        "haversine_km",
        4,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        move |ctx| {
            Ok(haversine_km(
                ctx.get::<f64>(0)?,
                ctx.get::<f64>(1)?,
                ctx.get::<f64>(2)?,
                ctx.get::<f64>(3)?,
            ))
        },
    )?;

    Ok(conn)
}

/// Find nearest villages using RTree spatial index + Haversine distance.
pub fn nearest(conn: &Connection, lat: f64, lon: f64, limit: usize) -> Result<Vec<Village>> {
    let limit = limit.min(20).max(1);

    let deltas: [f64; 10] = [0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 15.0, 45.0, 180.0];

    for &delta in &deltas {
        let sql = "
            SELECT l.kode, l.nama, l.kecamatan, l.kota, l.provinsi, l.lat, l.lon,
                   haversine_km(?1, ?2, l.lat, l.lon) AS dist
            FROM locations l
            JOIN geo_rtree r ON l.id = r.id
            WHERE r.min_lon <= ?4 AND r.max_lon >= ?3
              AND r.min_lat <= ?6 AND r.max_lat >= ?5
            ORDER BY dist
            LIMIT ?7
        ";

        let mut stmt = conn.prepare_cached(sql)?;
        let rows = stmt.query_map(
            rusqlite::params![
                lat,
                lon,
                lon - delta,
                lon + delta,
                lat - delta,
                lat + delta,
                limit as i64
            ],
            |row| {
                Ok(Village {
                    code: row.get(0)?,
                    name: row.get(1)?,
                    district: row.get(2)?,
                    city: row.get(3)?,
                    province: row.get(4)?,
                    lat: row.get(5)?,
                    lon: row.get(6)?,
                    dist_km: Some(row.get(7)?),
                })
            },
        )?;

        let results: Result<Vec<_>> = rows.collect();
        let results = results?;

        if results.len() >= limit {
            return Ok(results);
        }
    }

    Ok(vec![])
}

/// Search villages by name using FTS5 full-text search.
pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<Village>> {
    let limit = limit.min(100).max(1);

    let sql = "
        SELECT l.kode, l.nama, l.kecamatan, l.kota, l.provinsi, l.lat, l.lon
        FROM locations_fts f
        JOIN locations l ON f.rowid = l.id
        WHERE locations_fts MATCH ?1
        ORDER BY rank
        LIMIT ?2
    ";

    let mut stmt = conn.prepare_cached(sql)?;
    let rows = stmt.query_map(rusqlite::params![query, limit as i64], |row| {
        Ok(Village {
            code: row.get(0)?,
            name: row.get(1)?,
            district: row.get(2)?,
            city: row.get(3)?,
            province: row.get(4)?,
            lat: row.get(5)?,
            lon: row.get(6)?,
            dist_km: None,
        })
    })?;

    rows.collect()
}

/// A village record with administrative hierarchy and coordinates.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Village {
    /// BMKG-compatible administrative code (e.g., `31.71.03.1001`)
    pub code: String,
    /// Village (desa/kelurahan) name
    pub name: String,
    /// District (kecamatan) name
    pub district: String,
    /// City/regency (kabupaten/kota) name
    pub city: String,
    /// Province name
    pub province: String,
    /// Latitude coordinate
    pub lat: f64,
    /// Longitude coordinate
    pub lon: f64,
    /// Distance from query point in kilometers.
    /// Only set by `find_nearest()`, always `None` from `find_by_name()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist_km: Option<f64>,
}
