use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;
use tauri::{AppHandle, Manager, State};

/// SQLite is used directly rather than through `tauri-plugin-sql`: every
/// caller here is already Rust-side (templates, config, history, and the job
/// runner), so routing persistence through the frontend would add an IPC hop
/// and put raw SQL in the webview for no benefit.
pub struct Db(pub Mutex<Connection>);

pub type DbState<'a> = State<'a, Db>;

/// Bumped whenever the schema changes; `migrate` applies each step in order.
/// Stored in SQLite's own `user_version` pragma, so there's no bootstrap
/// table to create before it can be read.
const SCHEMA_VERSION: i64 = 2;

pub fn db_path(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .expect("app data dir is unavailable");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("ytdlpty.sqlite3")
}

pub fn open(app: &AppHandle) -> Connection {
    let conn = Connection::open(db_path(app)).expect("failed to open the database");
    // WAL keeps a long-running read (the history list) from blocking a write
    // (a job finishing), which is the exact overlap this app produces.
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    let _ = conn.pragma_update(None, "foreign_keys", "ON");
    migrate(&conn);
    conn
}

/// True when `table` already has `column`. Used to make additive migrations
/// idempotent instead of trusting a version counter to be in step.
fn has_column(conn: &Connection, table: &str, column: &str) -> bool {
    let Ok(mut stmt) = conn.prepare(&format!("PRAGMA table_info({table})")) else {
        return false;
    };
    // Collected while `stmt` is still alive: the row iterator borrows it.
    let names: Vec<String> = match stmt.query_map([], |row| row.get::<_, String>(1)) {
        Ok(rows) => rows.filter_map(Result::ok).collect(),
        Err(_) => return false,
    };
    names.iter().any(|name| name == column)
}

fn migrate(conn: &Connection) {
    let current: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap_or(0);

    if current < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS templates (
                 id            TEXT PRIMARY KEY,
                 name          TEXT NOT NULL,
                 urls_default  TEXT NOT NULL DEFAULT '',
                 download_to   TEXT NOT NULL DEFAULT '',
                 parameters    TEXT NOT NULL DEFAULT '',
                 mode          TEXT NOT NULL DEFAULT 'raw',
                 next_seq      INTEGER NOT NULL DEFAULT 1,
                 created_at    TEXT NOT NULL,
                 order_index   INTEGER NOT NULL DEFAULT 0
             );

             CREATE TABLE IF NOT EXISTS config (
                 key   TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS history (
                 id            INTEGER PRIMARY KEY AUTOINCREMENT,
                 filename      TEXT,
                 filepath      TEXT,
                 platform      TEXT,
                 template_name TEXT,
                 url           TEXT NOT NULL,
                 started_at    TEXT,
                 finished_at   TEXT,
                 duration_ms   INTEGER,
                 size_bytes    INTEGER,
                 status        TEXT NOT NULL
             );

             CREATE INDEX IF NOT EXISTS history_finished_at
                 ON history (finished_at DESC);",
        )
        .expect("failed to create the schema");
    }

    // Checked against the real schema rather than gated on `user_version`.
    // The counter can legitimately run ahead of the tables — during
    // development a version bump and the migration it describes may land in
    // separate rebuilds, and once the counter says 2 a version-gated step
    // never runs again, leaving a column permanently missing while the code
    // selects it (which drops every row on read). Asking the table what it
    // actually has is self-healing and costs one pragma at startup.
    //
    // Additive on purpose: rows written before this survive with a NULL
    // command instead of being discarded.
    if !has_column(conn, "history", "command") {
        let _ = conn.execute("ALTER TABLE history ADD COLUMN command TEXT", []);
    }

    if current < SCHEMA_VERSION {
        let _ = conn.pragma_update(None, "user_version", SCHEMA_VERSION);
    }
}
