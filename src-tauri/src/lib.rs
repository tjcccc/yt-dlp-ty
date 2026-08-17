mod commands;
mod registry;
mod ytdlp;

use commands::binaries::{check_binary, update_ytdlp};
use commands::config::{get_config, set_config};
use commands::formats::probe_formats;
use commands::jobs::{cancel_all, cancel_job, start_downloads};
use commands::templates::{delete_template, list_templates, reorder_templates, save_template};
use registry::JobRegistry;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(JobRegistry::new())
        .invoke_handler(tauri::generate_handler![
            check_binary,
            update_ytdlp,
            get_config,
            set_config,
            list_templates,
            save_template,
            delete_template,
            reorder_templates,
            probe_formats,
            start_downloads,
            cancel_job,
            cancel_all,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
