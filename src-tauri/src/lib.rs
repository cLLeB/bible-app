mod books;
mod commands;
mod db;
mod events;
mod reference;

use commands::AppState;
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

            // Seed WEB once (idempotent). data/web.canonical.json is bundled as a resource.
            if let Ok(seed_path) = app
                .path()
                .resolve("web.canonical.json", tauri::path::BaseDirectory::Resource)
            {
                if let Ok(json) = std::fs::read_to_string(&seed_path) {
                    db.seed_from_json(&json).expect("seed");
                }
            }

            app.manage(AppState { db: Mutex::new(db), translation: "WEB".into() });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::lookup_reference,
            commands::project_verse,
            commands::blank_projection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
