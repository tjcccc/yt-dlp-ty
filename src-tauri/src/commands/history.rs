use rusqlite::{params, Connection};
use serde::Serialize;

use crate::db::DbState;

/// One finished download. Deliberately narrow: this is a record of what was
/// fetched, not a media library. `filepath` is kept for provenance (proving
/// which file a row refers to when filenames repeat) but is never surfaced
/// as an open/reveal action — the app is a downloader, not a player.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub filename: Option<String>,
    pub filepath: Option<String>,
    pub platform: Option<String>,
    pub template_name: Option<String>,
    pub url: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub size_bytes: Option<i64>,
    pub status: String,
    /// The shell-quoted invocation that produced this file. Rows recorded
    /// before the column existed carry NULL.
    pub command: Option<String>,
}

/// What the job runner hands over when a download reaches a terminal state.
pub struct NewEntry {
    pub filename: Option<String>,
    pub filepath: Option<String>,
    pub platform: Option<String>,
    pub template_name: Option<String>,
    pub url: String,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: i64,
    pub size_bytes: Option<i64>,
    pub status: String,
    pub command: Option<String>,
}

/// Records every terminal outcome, including failures and cancellations.
/// The view only lists completed downloads, but storing the rest means a
/// "why did that never arrive?" question is answerable later instead of
/// having been discarded at the moment it happened.
pub fn record(conn: &Connection, entry: &NewEntry) {
    let _ = conn.execute(
        "INSERT INTO history
             (filename, filepath, platform, template_name, url,
              started_at, finished_at, duration_ms, size_bytes, status, command)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            entry.filename,
            entry.filepath,
            entry.platform,
            entry.template_name,
            entry.url,
            entry.started_at,
            entry.finished_at,
            entry.duration_ms,
            entry.size_bytes,
            entry.status,
            entry.command,
        ],
    );
}

#[tauri::command]
pub fn list_history(db: DbState<'_>, limit: Option<i64>, offset: Option<i64>) -> Vec<HistoryEntry> {
    let conn = db.0.lock().unwrap();
    // Only completed downloads are listed. Failures live in the table but
    // showing them would need a status column the user explicitly didn't
    // ask for, and a failed row has no filename or size to fill the ones
    // that exist.
    let Ok(mut stmt) = conn.prepare(
        "SELECT * FROM history
         WHERE status = 'completed'
         ORDER BY finished_at DESC, id DESC
         LIMIT ?1 OFFSET ?2",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map(
        params![limit.unwrap_or(500), offset.unwrap_or(0)],
        |row| {
            Ok(HistoryEntry {
                id: row.get("id")?,
                filename: row.get("filename")?,
                filepath: row.get("filepath")?,
                platform: row.get("platform")?,
                template_name: row.get("template_name")?,
                url: row.get("url")?,
                started_at: row.get("started_at")?,
                finished_at: row.get("finished_at")?,
                duration_ms: row.get("duration_ms")?,
                size_bytes: row.get("size_bytes")?,
                status: row.get("status")?,
                command: row.get("command")?,
            })
        },
    ) else {
        return Vec::new();
    };
    rows.filter_map(Result::ok).collect()
}

#[tauri::command]
pub fn clear_history(db: DbState<'_>) {
    let conn = db.0.lock().unwrap();
    let _ = conn.execute("DELETE FROM history", []);
}
