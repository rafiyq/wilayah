//! Database metadata queries.

use rusqlite::Connection;

use crate::types::DataInfo;

fn query_meta(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM db_meta WHERE key = ?1", [key], |row| {
        row.get(0)
    })
    .ok()
}

pub(super) fn data_info_from_conn(conn: &Connection) -> DataInfo {
    DataInfo {
        source: query_meta(conn, "source").unwrap_or_else(|| "unknown".to_string()),
        decree: query_meta(conn, "decree").unwrap_or_else(|| "unknown".to_string()),
        village_count: query_meta(conn, "village_count")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        build_date: query_meta(conn, "build_date")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
    }
}
