use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const CONFIG_STORE: &str = "config.json";
const CONFIG_KEY: &str = "config";

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

/// Internal accessor reused by `resolve_configured` and job-spawning code —
/// not a Tauri command itself, so it can be called directly from Rust
/// without going through IPC.
pub fn load_config(app: &AppHandle) -> AppConfig {
    let Ok(store) = app.store(CONFIG_STORE) else {
        return AppConfig::default();
    };
    store
        .get(CONFIG_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn persist_config(app: &AppHandle, config: &AppConfig) {
    if let Ok(store) = app.store(CONFIG_STORE) {
        store.set(CONFIG_KEY, serde_json::to_value(config).unwrap());
        let _ = store.save();
    }
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> AppConfig {
    load_config(&app)
}

#[tauri::command]
pub fn set_config(app: AppHandle, config: AppConfig) -> AppConfig {
    persist_config(&app, &config);
    config
}
