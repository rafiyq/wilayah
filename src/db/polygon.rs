//! Polygon containment queries.

use std::collections::HashMap;

use rusqlite::Connection;

use super::Result;
use crate::geometry::{deserialize_vertices, haversine_km, point_in_polygon};
use crate::types::{location_from_village, LocateMethod, Location, Village};

pub(super) type VillageRingMap = HashMap<i64, Vec<(String, Vec<(f64, f64)>)>>;

pub(super) fn query_polygon_candidates(
    poly_conn: &Connection,
    lat: f64,
    lon: f64,
) -> Result<VillageRingMap> {
    let sql = "
        SELECT vp.village_id, vp.ring_type, vp.vertices
        FROM village_polygons vp
        WHERE vp.min_lon <= ?2 AND vp.max_lon >= ?1
        AND vp.min_lat <= ?4 AND vp.max_lat >= ?3
    ";

    let mut stmt = poly_conn.prepare_cached(sql)?;
    let rows = stmt.query_map(rusqlite::params![lon, lon, lat, lat], |row| {
        let village_id: i64 = row.get(0)?;
        let ring_type: String = row.get(1)?;
        let vertices_blob: Vec<u8> = row.get(2)?;
        Ok((village_id, ring_type, vertices_blob))
    })?;

    let mut village_rings = VillageRingMap::new();
    for row in rows {
        let (village_id, ring_type, blob) = row?;
        let vertices = deserialize_vertices(&blob).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, e.into())
        })?;
        village_rings
            .entry(village_id)
            .or_default()
            .push((ring_type, vertices));
    }

    Ok(village_rings)
}

pub(super) fn locate_contained(
    candidates: &VillageRingMap,
    conn: &Connection,
    lat: f64,
    lon: f64,
    by_id: impl Fn(&Connection, i64) -> Result<Option<Village>>,
) -> Result<Option<Location>> {
    for (village_id, rings) in candidates {
        let exteriors: Vec<&[(f64, f64)]> = rings
            .iter()
            .filter(|(rt, _)| rt == "exterior")
            .map(|(_, v)| v.as_slice())
            .collect();
        let interiors: Vec<&[(f64, f64)]> = rings
            .iter()
            .filter(|(rt, _)| rt == "interior")
            .map(|(_, v)| v.as_slice())
            .collect();

        for exterior in &exteriors {
            if point_in_polygon(lat, lon, exterior, &interiors) {
                let village = by_id(conn, *village_id)?;
                let Some(village) = village else {
                    continue;
                };
                let dist_km = haversine_km(lat, lon, village.lat, village.lon);
                if let Some(loc) = location_from_village(&village, dist_km, LocateMethod::Contained)
                {
                    return Ok(Some(loc));
                }
            }
        }
    }
    Ok(None)
}
