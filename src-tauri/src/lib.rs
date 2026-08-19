mod commands;
mod db;
mod registry;
mod ytdlp;

use commands::binaries::{check_binary, update_ytdlp};
use commands::config::{get_config, set_config};
use commands::formats::{cancel_probe, probe_formats, ProbeEpoch};
use commands::history::{clear_history, list_history};
use commands::jobs::{cancel_all, cancel_job, start_downloads};
use commands::templates::{delete_template, list_templates, reorder_templates, save_template};
use db::Db;
use registry::JobRegistry;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Opened once at startup rather than per command: the schema
            // migration must run before anything queries, and a single
            // connection behind a mutex keeps writes serialized.
            let conn = db::open(app.handle());
            app.manage(Db(std::sync::Mutex::new(conn)));
            Ok(())
        })
        .manage(JobRegistry::new())
        .manage(ProbeEpoch::default())
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
            cancel_probe,
            start_downloads,
            cancel_job,
            cancel_all,
            list_history,
            clear_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
