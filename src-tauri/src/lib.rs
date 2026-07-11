mod books;
mod commands;
mod db;
mod events;
mod reference;
mod slides;

use commands::AppState;
use events::ProjectionState;
use std::sync::Mutex;
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

            app.manage(AppState {
                db: Mutex::new(db),
                translation: "WEB".into(),
                current: Mutex::new(ProjectionState::Blank),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::lookup_reference,
            commands::get_projection,
            commands::project_verse,
            commands::project_slide,
            commands::blank_projection,
            commands::add_song,
            commands::list_songs,
            commands::get_song_slides,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
