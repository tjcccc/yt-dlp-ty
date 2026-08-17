use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;
use uuid::Uuid;

const TEMPLATES_STORE: &str = "templates.json";
const TEMPLATES_KEY: &str = "templates";

const DEFAULT_DOWNLOAD_TO: &str =
    "~/Downloads/yt-dlp/{date:YYYY-MM-DD}_{id:NNN}_{id:guid}_{original_filename}";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsConfig {
    /// "raw" | "bestVideo" | "bestAudio" | "chooseFormat" — a mutually
    /// exclusive strategy, not independent booleans. "chooseFormat" is
    /// accepted here for forward-compatibility with Milestone 3, but the
    /// current UI keeps that toggle disabled since its real behavior
    /// (probing formats, showing a picker) isn't built yet.
    pub mode: String,
}

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

pub fn load_templates(app: &AppHandle) -> Vec<Template> {
    let Ok(store) = app.store(TEMPLATES_STORE) else {
        return seed_templates();
    };
    match store.get(TEMPLATES_KEY) {
        Some(value) => serde_json::from_value(value).unwrap_or_else(|_| seed_templates()),
        None => {
            let seeded = seed_templates();
            store.set(TEMPLATES_KEY, serde_json::to_value(&seeded).unwrap());
            let _ = store.save();
            seeded
        }
    }
}

fn save_templates(app: &AppHandle, templates: &[Template]) {
    if let Ok(store) = app.store(TEMPLATES_STORE) {
        store.set(TEMPLATES_KEY, serde_json::to_value(templates).unwrap());
        let _ = store.save();
    }
}

/// Consumes and increments a template's `{id:NNN}` counter at job-creation
/// time (not completion), so concurrent or cancelled jobs still get distinct
/// numbers. Returns the value the caller should use.
pub fn consume_next_seq(app: &AppHandle, template_id: &str) -> u32 {
    let mut templates = load_templates(app);
    let seq = match templates.iter_mut().find(|t| t.id == template_id) {
        Some(t) => {
            let seq = t.next_seq;
            t.next_seq += 1;
            seq
        }
        None => 1,
    };
    save_templates(app, &templates);
    seq
}

#[tauri::command]
pub fn list_templates(app: AppHandle) -> Vec<Template> {
    load_templates(&app)
}

#[tauri::command]
pub fn save_template(app: AppHandle, mut template: Template) -> Template {
    let mut templates = load_templates(&app);
    if template.id.is_empty() {
        template.id = Uuid::new_v4().to_string();
    }
    match templates.iter_mut().find(|t| t.id == template.id) {
        Some(existing) => {
            // `next_seq` is owned here, not by the caller: it advances on
            // every job spawn (`consume_next_seq`), so a UI copy read when
            // the form was opened is already stale by the time the user
            // saves. Writing it back would rewind the counter and collide
            // filenames with files already on disk.
            template.next_seq = existing.next_seq;
            *existing = template.clone();
        }
        None => templates.push(template.clone()),
    }
    save_templates(&app, &templates);
    template
}

#[tauri::command]
pub fn delete_template(app: AppHandle, id: String) {
    let mut templates = load_templates(&app);
    templates.retain(|t| t.id != id);
    save_templates(&app, &templates);
}

#[tauri::command]
pub fn reorder_templates(app: AppHandle, ids: Vec<String>) {
    let mut templates = load_templates(&app);
    let order_index: std::collections::HashMap<&String, usize> =
        ids.iter().enumerate().map(|(i, id)| (id, i)).collect();
    templates.sort_by_key(|t| order_index.get(&t.id).copied().unwrap_or(usize::MAX));
    for (i, t) in templates.iter_mut().enumerate() {
        t.order = i as i32;
    }
    save_templates(&app, &templates);
}
