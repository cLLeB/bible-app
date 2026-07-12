mod audio;
mod books;
mod commands;
mod db;
mod detect;
mod events;
mod reference;
mod remote;
mod semantic;
mod slides;
mod stt;

use commands::AppState;
use events::{ProjectionSettings, ProjectionState};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Resolve app data dir; open + migrate + seed on first run.
            let data_dir = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&data_dir).ok();
            let db_path = data_dir.join("bible.sqlite");
            let db = db::open_at(&db_path).expect("open db");
            db.migrate().expect("migrate");

            // Seed WEB once (idempotent). In a bundled build the data file is a
            // resource; in `tauri dev` the resource dir isn't populated, so fall
            // back to the project `data/` dir (resolved at compile time).
            let mut seed_json: Option<String> = app
                .path()
                .resolve("web.canonical.json", tauri::path::BaseDirectory::Resource)
                .ok()
                .and_then(|p| std::fs::read_to_string(&p).ok());
            if seed_json.is_none() {
                let dev_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../data/web.canonical.json");
                seed_json = std::fs::read_to_string(&dev_path).ok();
            }
            if let Some(json) = seed_json {
                db.seed_from_json(&json).expect("seed");
            }
            // Seed every *.canonical.json in the project data dir (multi-translation).
            let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
            if let Ok(entries) = std::fs::read_dir(&data_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path
                        .file_name()
                        .map(|n| n.to_string_lossy().ends_with(".canonical.json"))
                        .unwrap_or(false)
                    {
                        if let Ok(json) = std::fs::read_to_string(&path) {
                            let _ = db.seed_from_json(&json);
                        }
                    }
                }
            }
            db.seed_default_songs(include_str!("../default-songs.json"), 1)
                .expect("seed default songs");
            db.sync_fts().expect("sync fts");

            app.manage(AppState {
                db: Mutex::new(db),
                translation: Mutex::new("WEB".into()),
                current: Mutex::new(ProjectionState::Blank),
                settings: Mutex::new(ProjectionSettings::default()),
                listening: Arc::new(AtomicBool::new(false)),
                remote_running: Arc::new(AtomicBool::new(false)),
                cursor: Mutex::new(None),
            });

            // Closing the projection/stage windows should hide them, not
            // destroy them, so they can always be revealed again.
            for label in ["projection", "stage"] {
                if let Some(win) = app.get_webview_window(label) {
                    let hide_target = win.clone();
                    win.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();
                            let _ = hide_target.hide();
                        }
                    });
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::lookup_reference,
            commands::search_scripture,
            commands::list_translations,
            commands::get_translation,
            commands::set_translation,
            commands::get_projection,
            commands::project_verse,
            commands::project_slide,
            commands::present_coords,
            commands::navigate,
            commands::blank_projection,
            commands::set_projection,
            commands::show_stage,
            commands::get_projection_settings,
            commands::set_projection_settings,
            commands::add_song,
            commands::list_songs,
            commands::get_song_slides,
            commands::get_song,
            commands::update_song,
            commands::delete_song,
            commands::export_songs,
            commands::import_songs,
            commands::start_listening,
            commands::stop_listening,
            commands::start_remote,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
