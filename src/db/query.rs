//! SQL query functions for village lookups.

use rusqlite::Connection;

use super::{Error, Result, VILLAGE_COLS, VILLAGE_COLS_L};
use crate::types::{
    location_from_village, LocateMethod, Location, LookupResult, PrefixResult, Village,
    CODE_PREFIX_MAX_LIMIT, NEAREST_MAX_LIMIT, SEARCH_MAX_LIMIT,
};

fn collect_rows<T>(
    rows: rusqlite::MappedRows<impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>> {
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Error::from)
}

pub(super) fn village_from_row(
    row: &rusqlite::Row<'_>,
    dist_col: bool,
) -> rusqlite::Result<Village> {
    Ok(Village {
        code: row.get(0)?,
        name: row.get(1)?,
        district: row.get(2)?,
        city: row.get(3)?,
        province: row.get(4)?,
        lat: row.get(5)?,
        lon: row.get(6)?,
        dist_km: if dist_col { Some(row.get(7)?) } else { None },
    })
}

fn village_by_field<P: rusqlite::types::ToSql>(
    conn: &Connection,
    sql: &str,
    param: P,
) -> Result<Option<Village>> {
    let mut stmt = conn.prepare_cached(sql)?;
    let mut rows = stmt.query_map(rusqlite::params![param], |row| village_from_row(row, false))?;
    match rows.next() {
        Some(Ok(v)) => Ok(Some(v)),
        Some(Err(e)) => Err(Error::from(e)),
        None => Ok(None),
    }
}

pub(super) fn nearest(conn: &Connection, lat: f64, lon: f64, limit: usize) -> Result<Vec<Village>> {
    let limit = limit.clamp(1, NEAREST_MAX_LIMIT);

    let deltas: [f64; 10] = [0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 15.0, 45.0, 180.0];

    for &delta in &deltas {
        let sql = format!(
            "SELECT {VILLAGE_COLS_L},
            haversine_km(?1, ?2, l.lat, l.lon) AS dist
            FROM locations l
            JOIN geo_rtree r ON l.id = r.id
            WHERE r.min_lon <= ?4 AND r.max_lon >= ?3
            AND r.min_lat <= ?6 AND r.max_lat >= ?5
            ORDER BY dist
            LIMIT ?7"
        );

        let mut stmt = conn.prepare_cached(&sql)?;
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
            |row| village_from_row(row, true),
        )?;

        let results: Vec<Village> = collect_rows(rows)?;

        if results.len() >= limit {
            return Ok(results);
        }
    }

    Ok(vec![])
}

pub(super) fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<Village>> {
    let limit = limit.clamp(1, SEARCH_MAX_LIMIT);

    let sql = format!(
        "SELECT {VILLAGE_COLS_L}
        FROM locations_fts f
        JOIN locations l ON f.rowid = l.id
        WHERE locations_fts MATCH ?1
        ORDER BY rank
        LIMIT ?2"
    );

    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(rusqlite::params![query, limit as i64], |row| {
        village_from_row(row, false)
    })?;

    collect_rows(rows)
}

pub(super) fn by_code(conn: &Connection, code: &str) -> Result<Option<Village>> {
    village_by_field(
        conn,
        &format!("SELECT {VILLAGE_COLS} FROM locations WHERE kode = ?1"),
        code,
    )
}

pub(super) fn by_code_prefix(
    conn: &Connection,
    prefix: &str,
    limit: usize,
    offset: usize,
) -> Result<PrefixResult> {
    let limit = limit.clamp(1, CODE_PREFIX_MAX_LIMIT);
    let pattern = format!("{}%", prefix);

    let total_i64: i64 = conn.query_row(
        "SELECT COUNT(*) FROM locations WHERE kode LIKE ?1",
        [&pattern],
        |row| row.get(0),
    )?;
    let total = total_i64 as usize;

    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {VILLAGE_COLS}
        FROM locations
        WHERE kode LIKE ?1
        ORDER BY kode
        LIMIT ?2
        OFFSET ?3"
    ))?;
    let rows = stmt.query_map(
        rusqlite::params![pattern, limit as i64, offset as i64],
        |row| village_from_row(row, false),
    )?;
    let villages: Vec<Village> = collect_rows(rows)?;

    let has_more = offset + villages.len() < total;

    Ok(PrefixResult {
        villages,
        total,
        has_more,
    })
}

pub(super) fn search_unique(conn: &Connection, query: &str) -> Result<LookupResult> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {VILLAGE_COLS_L}
        FROM locations_fts f
        JOIN locations l ON f.rowid = l.id
        WHERE locations_fts MATCH ?1
        ORDER BY rank
        LIMIT 20"
    ))?;
    let rows = stmt.query_map(rusqlite::params![query], |row| village_from_row(row, false))?;
    let results: Vec<_> = collect_rows(rows)?;

    Ok(match results.len() {
        0 => LookupResult::NotFound,
        1 => LookupResult::Found(results.into_iter().next().unwrap()),
        _ => LookupResult::Ambiguous(results),
    })
}

pub(super) fn locate_nearest(conn: &Connection, lat: f64, lon: f64) -> Result<Option<Location>> {
    let mut results = nearest(conn, lat, lon, 1)?;
    let village = match results.pop() {
        Some(v) => v,
        None => return Ok(None),
    };

    let dist_km = village.dist_km.unwrap_or(0.0);
    Ok(location_from_village(
        &village,
        dist_km,
        LocateMethod::Nearest,
    ))
}

pub(super) fn by_id(conn: &Connection, id: i64) -> Result<Option<Village>> {
    village_by_field(
        conn,
        &format!("SELECT {VILLAGE_COLS} FROM locations WHERE id = ?1"),
        id,
    )
}
