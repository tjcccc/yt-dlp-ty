use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use uuid::Uuid;

use crate::db::DbState;

const DEFAULT_DOWNLOAD_TO: &str =
    "~/Downloads/yt-dlp/{date:YYYY-MM-DD}_{id:NNN}_{id:guid}_{original_filename}";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsConfig {
    /// "raw" | "bestVideo" | "bestAudio" | "chooseFormat" — a mutually
    /// exclusive strategy, not independent booleans.
    pub mode: String,
}

/// Wire shape is unchanged from the JSON-store era on purpose: the frontend
/// contract stays identical, so moving storage to SQLite touched no
/// TypeScript. `order` is `order_index` in SQL only because ORDER is a
/// reserved word.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub name: String,
    pub urls_default: String,
    pub download_to: String,
    pub parameters: String,
    pub options: OptionsConfig,
    pub next_seq: u32,
    pub created_at: String,
    pub order: i32,
}

/// Seed templates use corrected example Parameters vs. the original mockup:
/// `--cookies-from-browser chrome` (the mockup's `--cookies chrome` isn't a
/// valid flag — `--cookies` takes a Netscape cookie *file*), and no `-U`
/// (self-update doesn't belong per-job — see DEVLOG "Bugfix" entries).
fn seed_templates() -> Vec<Template> {
    let make = |name: &str, order: i32| Template {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        urls_default: String::new(),
        download_to: DEFAULT_DOWNLOAD_TO.to_string(),
        parameters: "--no-playlist --cookies-from-browser chrome".to_string(),
        options: OptionsConfig {
            mode: "raw".to_string(),
        },
        next_seq: 1,
        created_at: chrono::Utc::now().to_rfc3339(),
        order,
    };
    vec![make("YouTube", 0), make("Bilibili", 1), make("TikTok", 2)]
}

fn row_to_template(row: &rusqlite::Row<'_>) -> rusqlite::Result<Template> {
    Ok(Template {
        id: row.get("id")?,
        name: row.get("name")?,
        urls_default: row.get("urls_default")?,
        download_to: row.get("download_to")?,
        parameters: row.get("parameters")?,
        options: OptionsConfig {
            mode: row.get("mode")?,
        },
        next_seq: row.get::<_, i64>("next_seq")? as u32,
        created_at: row.get("created_at")?,
        order: row.get::<_, i64>("order_index")? as i32,
    })
}

/// Note the deliberate omission of `next_seq` from the DO UPDATE clause —
/// see `save_template`.
fn upsert(conn: &Connection, t: &Template) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO templates
             (id, name, urls_default, download_to, parameters, mode, next_seq, created_at, order_index)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
             name = excluded.name,
             urls_default = excluded.urls_default,
             download_to = excluded.download_to,
             parameters = excluded.parameters,
             mode = excluded.mode,
             created_at = excluded.created_at,
             order_index = excluded.order_index",
        params![
            t.id,
            t.name,
            t.urls_default,
            t.download_to,
            t.parameters,
            t.options.mode,
            t.next_seq as i64,
            t.created_at,
            t.order as i64,
        ],
    )?;
    Ok(())
}

/// Imports the pre-SQLite `templates.json` written by `tauri-plugin-store`.
///
/// The JSON file is deliberately left on disk rather than deleted: it is the
/// only other copy of the user's tuned templates, and keeping it costs a few
/// KB. `next_seq` carries across verbatim — it drives `{id:NNN}` in output
/// paths, so restarting it at 1 would generate filenames colliding with files
/// already downloaded.
fn import_legacy_json(app: &AppHandle, conn: &Connection) -> Option<Vec<Template>> {
    let path = crate::db::db_path(app).with_file_name("templates.json");
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    // The store nests under a "templates" key; tolerate a bare array too.
    let array = value.get("templates").cloned().unwrap_or(value);
    let templates: Vec<Template> = serde_json::from_value(array).ok()?;
    if templates.is_empty() {
        return None;
    }
    for t in &templates {
        let _ = upsert(conn, t);
    }
    Some(templates)
}

fn read_all(conn: &Connection) -> Vec<Template> {
    let Ok(mut stmt) = conn.prepare("SELECT * FROM templates ORDER BY order_index ASC") else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], row_to_template) else {
        return Vec::new();
    };
    rows.filter_map(Result::ok).collect()
}

/// Reads every template, migrating or seeding on first use.
///
/// Precedence is deliberate: existing rows win, then the legacy JSON, and
/// only a genuinely fresh install falls through to the built-in seeds. An
/// upgrade must never look like a fresh install and silently replace tuned
/// templates with the three defaults.
pub fn load_templates(app: &AppHandle, conn: &Connection) -> Vec<Template> {
    let existing = read_all(conn);
    if !existing.is_empty() {
        return existing;
    }
    if let Some(imported) = import_legacy_json(app, conn) {
        return imported;
    }
    let seeded = seed_templates();
    for t in &seeded {
        let _ = upsert(conn, t);
    }
    seeded
}

/// Consumes and increments a template's `{id:NNN}` counter at job-creation
/// time (not completion), so concurrent or cancelled jobs still get distinct
/// numbers.
///
/// One `UPDATE ... RETURNING` keeps this atomic: a batch spawns jobs from
/// several threads, and a read-modify-write would hand the same number to
/// two of them.
pub fn consume_next_seq(conn: &Connection, template_id: &str) -> u32 {
    conn.query_row(
        "UPDATE templates SET next_seq = next_seq + 1
         WHERE id = ?1
         RETURNING next_seq - 1",
        params![template_id],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .ok()
    .flatten()
    .map(|v| v as u32)
    .unwrap_or(1)
}

/// Snapshotted onto each job so history keeps the name the template had when
/// the download ran, even if it is renamed or deleted afterwards.
pub fn template_name(conn: &Connection, template_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT name FROM templates WHERE id = ?1",
        params![template_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
}

#[tauri::command]
pub fn list_templates(app: AppHandle, db: DbState<'_>) -> Vec<Template> {
    let conn = db.0.lock().unwrap();
    load_templates(&app, &conn)
}

#[tauri::command]
pub fn save_template(app: AppHandle, db: DbState<'_>, mut template: Template) -> Template {
    let conn = db.0.lock().unwrap();
    load_templates(&app, &conn);
    if template.id.is_empty() {
        template.id = Uuid::new_v4().to_string();
    }
    // `next_seq` is owned by the backend, not the caller: it advances on
    // every job spawn (`consume_next_seq`), so the UI's copy is already stale
    // by the time the user presses Save. `upsert` never writes it on
    // conflict for that reason; echo the stored value back so the frontend
    // re-syncs instead of holding a number that would rewind the counter.
    let stored_seq: Option<i64> = conn
        .query_row(
            "SELECT next_seq FROM templates WHERE id = ?1",
            params![template.id],
            |row| row.get(0),
        )
        .optional()
        .ok()
        .flatten();
    if let Some(seq) = stored_seq {
        template.next_seq = seq as u32;
    }
    let _ = upsert(&conn, &template);
    template
}

#[tauri::command]
pub fn delete_template(db: DbState<'_>, id: String) {
    let conn = db.0.lock().unwrap();
    let _ = conn.execute("DELETE FROM templates WHERE id = ?1", params![id]);
}

#[tauri::command]
pub fn reorder_templates(db: DbState<'_>, ids: Vec<String>) {
    let mut conn = db.0.lock().unwrap();
    let Ok(tx) = conn.transaction() else { return };
    for (i, id) in ids.iter().enumerate() {
        let _ = tx.execute(
            "UPDATE templates SET order_index = ?1 WHERE id = ?2",
            params![i as i64, id],
        );
    }
    let _ = tx.commit();
}
