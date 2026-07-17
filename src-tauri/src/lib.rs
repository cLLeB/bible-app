pub mod audio;
pub mod books;
pub mod calibrate;
mod capture;
mod commands;
pub mod corrections;
mod flavor;
pub mod db;
pub mod detect;
mod events;
pub mod idle;
pub mod knowledge;
pub mod learn;
pub mod pjlink;
pub mod profile_seed;
pub mod reference;
pub mod relearn;
mod remote;
mod resolution;
pub mod sessions;
mod translations;
pub mod semantic;
mod slides;
pub mod stt;
pub mod themes;

use commands::AppState;
use events::ProjectionState;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

/// Collect every `*.canonical.json` under `dir` (recursively, shallow).
fn collect_canonical_translations(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
    if depth > 2 {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_canonical_translations(&path, depth + 1, out);
            } else if path
                .file_name()
                .map(|n| n.to_string_lossy().ends_with(".canonical.json"))
                .unwrap_or(false)
            {
                out.push(path);
            }
        }
    }
}

/// A file is worth opening only if we haven't already seeded that exact file.
/// Without this every launch re-parses ~6 MB of JSON per bundled translation
/// just to discover the verses are already in the DB.
fn seed_key(path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    format!("seed:{name}:{len}")
}

/// Seed bundled translations and songs into the DB. Runs on a background thread
/// and holds the db lock throughout, so it must never run before `manage()`.
fn seed_library(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Packaged builds carry the translations as resources; `tauri dev` reads the
    // project `data/` dir (resolved at compile time). The flavor build decides
    // which translation files are present.
    let dev_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
    let mut files = Vec::new();
    if let Ok(res_dir) = app.path().resource_dir() {
        collect_canonical_translations(&res_dir, 0, &mut files);
    }
    collect_canonical_translations(&dev_dir, 0, &mut files);

    let pending: Vec<PathBuf> =
        files.into_iter().filter(|p| db.get_setting(&seed_key(p)).is_none()).collect();
    for (i, path) in pending.iter().enumerate() {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().replace(".canonical.json", "").to_uppercase())
            .unwrap_or_default();
        let _ = app.emit(
            "library-progress",
            format!("Installing {name}… ({} of {})", i + 1, pending.len()),
        );
        let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        db.seed_from_json(&json).map_err(|e| e.to_string())?;
        db.set_setting(&seed_key(path), "1").map_err(|e| e.to_string())?;
    }

    let _ = app.emit("library-progress", "Adding songs…".to_string());
    db.seed_default_songs(include_str!("../default-songs.json"), 1)
        .map_err(|e| e.to_string())?;
    // Personal-tier only: the operator's own song library (gitignored, never
    // distributed). Seeded from data/personal.songs.json.
    if crate::flavor::is_personal() {
        for dir in [app.path().resource_dir().ok(), Some(dev_dir.clone())].into_iter().flatten() {
            if let Ok(json) = std::fs::read_to_string(dir.join("personal.songs.json")) {
                let _ = db.seed_personal_songs(&json, 1);
            }
        }
        // Arrive already tuned for this church's own preachers. Baked into the
        // binary at build time; guarded so it seeds once and never clobbers a
        // church's own calibration. Distribution builds skip it entirely.
        let _ = crate::profile_seed::apply(&db, crate::profile_seed::BAKED);
    }

    let _ = app.emit("library-progress", "Building the search index…".to_string());
    db.sync_fts().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // The webview windows already exist and are loading the frontend by
            // the time this hook runs, so `manage()` has to happen before any
            // slow work — otherwise the first commands the UI fires (list_songs
            // and friends) land before the state is registered and fail with
            // "state not managed". Opening + migrating is fast; seeding the
            // ~270 MB library is not, so it moves to a background thread below.
            let data_dir = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&data_dir).ok();
            let db_path = data_dir.join("bible.sqlite");
            let db = db::open_at(&db_path).expect("open db");
            db.migrate().expect("migrate");
            // Appearance persists across restarts (theme + font scale).
            let initial_settings = themes::load_projection_settings(&db);

            let ready = Arc::new(AtomicBool::new(false));
            app.manage(AppState {
                db: Mutex::new(db),
                translation: Mutex::new("WEB".into()),
                current: Mutex::new(ProjectionState::Blank),
                settings: Mutex::new(initial_settings),
                stage: Mutex::new(crate::events::StageInfo::default()),
                alert: Mutex::new(crate::events::Alert::default()),
                ready: ready.clone(),
                listening: Arc::new(AtomicBool::new(false)),
                recording: Arc::new(AtomicBool::new(false)),
                remote_running: Arc::new(AtomicBool::new(false)),
                cursor: Mutex::new(None),
                learned: Mutex::new(std::collections::HashMap::new()),
                moments: Mutex::new(Vec::new()),
                learning: Arc::new(AtomicBool::new(false)),
            });

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let result = seed_library(&handle);
                // Ready either way: a failed seed leaves a usable (if incomplete)
                // library, and the UI surfaces the error rather than hanging.
                ready.store(true, Ordering::Relaxed);
                match result {
                    Ok(()) => {
                        let _ = handle.emit("library-ready", ());
                    }
                    Err(e) => {
                        let _ = handle.emit("library-error", e);
                    }
                }
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

            // ...but that leaves them alive, and Tauri only exits once every
            // window is gone. So closing the console has to mean quit, or the
            // process lingers with no window at all — still holding the mic and
            // still transcribing, with no way to stop it but Task Manager.
            if let Some(main) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                main.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        let state = handle.state::<AppState>();
                        state.listening.store(false, Ordering::SeqCst);
                        state.remote_running.store(false, Ordering::SeqCst);
                        crate::stt::shutdown();
                        handle.exit(0);
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::lookup_reference,
            commands::search_scripture,
            commands::related_verses,
            commands::app_flavor,
            commands::library_ready,
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
            commands::song_usage_report,
            commands::present_coords,
            commands::project_parallel,
            commands::navigate,
            commands::blank_projection,
            commands::set_projection,
            commands::show_stage,
            commands::get_projection_settings,
            commands::list_themes,
            commands::set_active_theme,
            commands::set_font_scale,
            commands::save_theme,
            commands::delete_theme,
            commands::pjlink_command,
            commands::get_alert,
            commands::show_alert,
            commands::clear_alert,
            commands::get_stage,
            commands::set_stage,
            commands::set_stage_message,
            commands::set_stage_timer,
            commands::peek_next,
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
            commands::set_recording,
            commands::recording_enabled,
            commands::recording_consent,
            commands::set_recording_consent,
            commands::forget_recordings,
            commands::record_moment,
            commands::review_sessions,
            commands::learning_status,
            commands::learn_now,
            commands::stop_learning,
            commands::accept_proposal,
            commands::reject_proposal,
            commands::rollback_profile,
            commands::reset_profile_to_baked,
            commands::approve_session,
            commands::discard_session,
            commands::audio_inputs,
            commands::set_audio_input,
            commands::test_audio_input,
            commands::learn_from_recordings,
            commands::calibration_script,
            commands::voice_profiles,
            commands::set_voice_profile,
            commands::remove_voice_profile,
            commands::record_calibration_line,
            commands::run_calibration,
            commands::start_remote,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
