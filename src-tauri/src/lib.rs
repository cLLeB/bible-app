mod audio;
mod books;
mod commands;
mod corrections;
mod flavor;
mod db;
mod detect;
mod events;
mod knowledge;
mod reference;
mod remote;
mod resolution;
mod translations;
mod semantic;
mod slides;
mod stt;

use commands::AppState;
use events::{ProjectionSettings, ProjectionState};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Seed every `*.canonical.json` under `dir` (recursively, shallow) into the DB.
/// Idempotent, so scanning both the resource dir and the dev dir is safe.
fn seed_canonical_translations(db: &db::Db, dir: &std::path::Path, depth: u8) {
    if depth > 2 {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                seed_canonical_translations(db, &path, depth + 1);
            } else if path
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
}

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

            // Seed every bundled translation (*.canonical.json), idempotently.
            // Packaged builds carry them as resources; `tauri dev` reads the
            // project `data/` dir (resolved at compile time). The flavor build
            // decides which translation files are present.
            if let Ok(res_dir) = app.path().resource_dir() {
                seed_canonical_translations(&db, &res_dir, 0);
            }
            let dev_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
            seed_canonical_translations(&db, &dev_dir, 0);
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
                learned: Mutex::new(std::collections::HashMap::new()),
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
            commands::related_verses,
            commands::app_flavor,
            commands::translation_catalog,
            commands::download_translation,
            commands::chunk_passage,
            commands::record_choice,
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
