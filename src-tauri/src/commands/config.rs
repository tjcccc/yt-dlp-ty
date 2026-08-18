use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::db::{Db, DbState};

/// Stored as key/value rows rather than one JSON blob so a future setting can
/// be added without a migration, and so a partially-written value can never
/// take the whole config down with it.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub ytdlp_path: Option<String>,
    pub ffmpeg_path: Option<String>,
    pub proxy: String,
    pub concurrency: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ytdlp_path: None,
            ffmpeg_path: None,
            proxy: String::new(),
            concurrency: 3,
        }
    }
}

fn get(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM config WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
}

fn put(conn: &Connection, key: &str, value: &str) {
    let _ = conn.execute(
        "INSERT INTO config (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    );
}

fn read(conn: &Connection) -> AppConfig {
    let defaults = AppConfig::default();
    AppConfig {
        // An empty stored string means "no override", which is how the
        // Config page clears a custom binary path.
        ytdlp_path: get(conn, "ytdlpPath").filter(|s| !s.is_empty()),
        ffmpeg_path: get(conn, "ffmpegPath").filter(|s| !s.is_empty()),
        proxy: get(conn, "proxy").unwrap_or(defaults.proxy),
        concurrency: get(conn, "concurrency")
            .and_then(|s| s.parse().ok())
            .unwrap_or(defaults.concurrency),
    }
}

fn write(conn: &Connection, config: &AppConfig) {
    put(conn, "ytdlpPath", config.ytdlp_path.as_deref().unwrap_or(""));
    put(conn, "ffmpegPath", config.ffmpeg_path.as_deref().unwrap_or(""));
    put(conn, "proxy", &config.proxy);
    put(conn, "concurrency", &config.concurrency.to_string());
}

/// Imports the pre-SQLite `config.json`. Same precedence as templates:
/// existing rows win, then the legacy file, then defaults — so an upgrade
/// never silently resets a configured proxy or binary override.
fn import_legacy_json(app: &AppHandle, conn: &Connection) -> Option<AppConfig> {
    let path = crate::db::db_path(app).with_file_name("config.json");
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let inner = value.get("config").cloned().unwrap_or(value);
    let config: AppConfig = serde_json::from_value(inner).ok()?;
    write(conn, &config);
    Some(config)
}

fn load_from(app: &AppHandle, conn: &Connection) -> AppConfig {
    let has_rows: bool = conn
        .query_row("SELECT EXISTS(SELECT 1 FROM config)", [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|n| n != 0)
        .unwrap_or(false);
    if has_rows {
        return read(conn);
    }
    import_legacy_json(app, conn).unwrap_or_default()
}

/// Internal accessor reused by `resolve_configured` and job-spawning code —
/// not a Tauri command, so it can be called directly from Rust without an
/// IPC round trip. Takes the connection from managed state itself because
/// most callers only hold an `AppHandle`.
pub fn load_config(app: &AppHandle) -> AppConfig {
    let Some(db) = app.try_state::<Db>() else {
        return AppConfig::default();
    };
    let conn = db.0.lock().unwrap();
    load_from(app, &conn)
}

#[tauri::command]
pub fn get_config(app: AppHandle, db: DbState<'_>) -> AppConfig {
    let conn = db.0.lock().unwrap();
    load_from(&app, &conn)
}

#[tauri::command]
pub fn set_config(db: DbState<'_>, config: AppConfig) -> AppConfig {
    let conn = db.0.lock().unwrap();
    write(&conn, &config);
    config
}
