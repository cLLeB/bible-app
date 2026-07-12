use crate::books::book_by_osis;
use crate::db::{Db, SongSummary};
use crate::events::{ProjectionSettings, ProjectionState, VersePayload};
use crate::reference::parse_reference;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

pub struct AppState {
    pub db: Mutex<Db>,
    pub translation: Mutex<String>, // active translation code, e.g. "WEB"
    pub current: Mutex<ProjectionState>, // what the projection should show
    pub settings: Mutex<ProjectionSettings>, // display appearance
    pub listening: Arc<AtomicBool>,      // mic listen loop active?
    pub remote_running: Arc<AtomicBool>, // LAN remote server started?
}

impl AppState {
    pub fn active_translation(&self) -> String {
        self.translation.lock().map(|t| t.clone()).unwrap_or_else(|_| "WEB".into())
    }
}

pub(crate) fn build_payload(rec: crate::db::VerseRecord) -> VersePayload {
    let book_name = book_by_osis(&rec.book_osis)
        .map(|b| b.name.to_string())
        .unwrap_or_else(|| rec.book_osis.clone());
    let reference = format!("{} {}:{}", book_name, rec.chapter, rec.verse);
    VersePayload {
        reference,
        book: book_name,
        chapter: rec.chapter,
        verse: rec.verse,
        text: rec.text,
        translation: rec.translation,
    }
}

/// Pull a verse-range end off the query: "16-18", "16 to 18", "16 through 18".
fn extract_range(query: &str) -> (String, Option<u16>) {
    let lower = query.to_lowercase();
    for sep in [" through ", " thru ", " to ", "-"] {
        if let Some(pos) = lower.rfind(sep) {
            let after = lower[pos + sep.len()..].trim();
            if let Some(tok) = after.split_whitespace().next() {
                if let Ok(end) = tok.parse::<u16>() {
                    return (query[..pos].trim().to_string(), Some(end));
                }
            }
        }
    }
    (query.to_string(), None)
}

/// Reference lookup usable from commands and the LAN remote server.
pub(crate) fn do_lookup(state: &AppState, query: &str) -> Result<VersePayload, String> {
    let tr = state.active_translation();
    let (base_query, end) = extract_range(query);
    let parsed = parse_reference(&base_query).ok_or_else(|| format!("Could not parse '{query}'"))?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    if let (Some(start), Some(end)) = (parsed.verse, end) {
        if end > start {
            if let Some(text) = db
                .find_verse_range(&tr, &parsed.book_osis, parsed.chapter, start, end)
                .map_err(|e| e.to_string())?
            {
                let book = book_by_osis(&parsed.book_osis)
                    .map(|b| b.name.to_string())
                    .unwrap_or_else(|| parsed.book_osis.clone());
                return Ok(VersePayload {
                    reference: format!("{} {}:{}-{}", book, parsed.chapter, start, end),
                    book,
                    chapter: parsed.chapter,
                    verse: start,
                    text,
                    translation: tr,
                });
            }
        }
    }

    let rec = db
        .find_verse(&tr, &parsed)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Verse not found: '{query}'"))?;
    Ok(build_payload(rec))
}

#[tauri::command]
pub fn lookup_reference(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<VersePayload, String> {
    do_lookup(&state, &query)
}

#[tauri::command]
pub fn search_scripture(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<VersePayload>, String> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase())
        .filter(|w| w.len() >= 2)
        .map(|w| format!("\"{w}\""))
        .collect();
    if terms.is_empty() {
        return Ok(vec![]);
    }
    let fts = terms.join(" OR ");
    let tr = state.active_translation();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let hits = db.search_fts(&tr, &fts, 25).map_err(|e| e.to_string())?;
    Ok(hits.into_iter().map(|(rec, _)| build_payload(rec)).collect())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationInfo {
    pub code: String,
    pub name: String,
}

#[tauri::command]
pub fn list_translations(state: tauri::State<'_, AppState>) -> Result<Vec<TranslationInfo>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let list = db.list_translations().map_err(|e| e.to_string())?;
    Ok(list.into_iter().map(|(code, name)| TranslationInfo { code, name }).collect())
}

#[tauri::command]
pub fn get_translation(state: tauri::State<'_, AppState>) -> String {
    state.active_translation()
}

#[tauri::command]
pub fn set_translation(code: String, state: tauri::State<'_, AppState>) {
    if let Ok(mut t) = state.translation.lock() {
        *t = code;
    }
}

// ---- Songs ----

#[tauri::command]
pub fn add_song(
    title: String,
    author: Option<String>,
    lyrics: String,
    state: tauri::State<'_, AppState>,
) -> Result<i64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_song(&title, author.as_deref(), &lyrics)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_songs(state: tauri::State<'_, AppState>) -> Result<Vec<SongSummary>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.list_songs().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_song_slides(
    song_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::db::SlideRecord>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_song_slides(song_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_song(
    song_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<Option<crate::db::SongDetail>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_song(song_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_song(
    song_id: i64,
    title: String,
    author: Option<String>,
    lyrics: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.update_song(song_id, &title, author.as_deref(), &lyrics)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_song(song_id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_song(song_id).map_err(|e| e.to_string())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SongExport {
    title: String,
    author: Option<String>,
    lyrics: String,
}

#[tauri::command]
pub fn export_songs(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let songs = db.all_songs_full().map_err(|e| e.to_string())?;
    let export: Vec<SongExport> = songs
        .into_iter()
        .map(|s| SongExport { title: s.title, author: s.author, lyrics: s.lyrics })
        .collect();
    serde_json::to_string_pretty(&export).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_songs(json: String, state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let items: Vec<SongExport> =
        serde_json::from_str(&json).map_err(|e| format!("Invalid songs file: {e}"))?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut n = 0;
    for it in &items {
        db.add_song(&it.title, it.author.as_deref(), &it.lyrics)
            .map_err(|e| e.to_string())?;
        n += 1;
    }
    Ok(n)
}

// ---- Projection ----

/// Called by the projection window on mount to get the current state,
/// avoiding the race where the window opens after an emit has fired.
#[tauri::command]
pub fn get_projection(state: tauri::State<'_, AppState>) -> ProjectionState {
    state
        .current
        .lock()
        .map(|c| c.clone())
        .unwrap_or(ProjectionState::Blank)
}

/// The projection window is declared (hidden) in tauri.conf.json, so it loads
/// index.html at startup exactly like the main window. Here we just position,
/// size, and reveal it.
fn ensure_projection_window(app: &tauri::AppHandle) -> Result<(), String> {
    let win = app
        .get_webview_window("projection")
        .ok_or_else(|| "projection window not found".to_string())?;

    // Move to the second monitor if present, else stay on primary.
    if let Ok(monitors) = win.available_monitors() {
        if let Some(second) = monitors.get(1) {
            let pos = second.position();
            win.set_position(tauri::PhysicalPosition { x: pos.x, y: pos.y })
                .map_err(|e| e.to_string())?;
        }
    }
    win.set_fullscreen(true).map_err(|e| e.to_string())?;
    win.show().map_err(|e| e.to_string())?;
    win.set_focus().ok();
    Ok(())
}

/// Set projection state from an app handle (usable off the command path, e.g.
/// the LAN remote server thread).
pub(crate) fn project_via_handle(
    app: &tauri::AppHandle,
    next: ProjectionState,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    if let Ok(mut cur) = state.current.lock() {
        *cur = next.clone();
    }
    ensure_projection_window(app)?;
    app.emit_to("projection", "set-projection", next)
        .map_err(|e| e.to_string())
}

fn project(
    app: &tauri::AppHandle,
    _state: &tauri::State<'_, AppState>,
    next: ProjectionState,
) -> Result<(), String> {
    project_via_handle(app, next)
}

#[tauri::command]
pub fn project_verse(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    payload: VersePayload,
) -> Result<(), String> {
    let caption = format!("{} · {}", payload.reference, payload.translation);
    project(&app, &state, ProjectionState::Verse { text: payload.text, caption })
}

#[tauri::command]
pub fn project_slide(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    song_id: i64,
    index: usize,
) -> Result<(), String> {
    let (text, caption) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let slides = db.get_song_slides(song_id).map_err(|e| e.to_string())?;
        let slide = slides.get(index).ok_or_else(|| "slide index out of range".to_string())?;
        let title = db
            .get_song_title(song_id)
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| "Song".to_string());
        (
            slide.text.clone(),
            format!("{} ({}/{})", title, index + 1, slides.len()),
        )
    };
    project(&app, &state, ProjectionState::Song { text, caption })
}

#[tauri::command]
pub fn blank_projection(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    project(&app, &state, ProjectionState::Blank)
}

/// Generic projection setter for Blackout / Logo / Message / Countdown, etc.
#[tauri::command]
pub fn set_projection(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    next: ProjectionState,
) -> Result<(), String> {
    project(&app, &state, next)
}

#[tauri::command]
pub fn show_stage(app: tauri::AppHandle) -> Result<(), String> {
    let win = app
        .get_webview_window("stage")
        .ok_or_else(|| "stage window not found".to_string())?;
    win.show().map_err(|e| e.to_string())?;
    win.set_focus().ok();
    Ok(())
}

#[tauri::command]
pub fn get_projection_settings(state: tauri::State<'_, AppState>) -> ProjectionSettings {
    state.settings.lock().map(|s| s.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn set_projection_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    settings: ProjectionSettings,
) -> Result<(), String> {
    if let Ok(mut s) = state.settings.lock() {
        *s = settings.clone();
    }
    app.emit_to("projection", "set-settings", settings)
        .map_err(|e| e.to_string())
}

// ---- Live listening (STT) ----

fn first_existing(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.exists())
}

/// Locate the whisper model + binary. Dev: project `models/` and `bin/` dirs.
/// `kind` selects the flavor: "base" (normal) or "tiny" (low-end PCs).
fn resolve_model_and_binary(kind: &str) -> Result<(PathBuf, PathBuf), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("bad project root")?
        .to_path_buf();
    let models = root.join("models");

    let requested = models.join(format!("ggml-{kind}.en.bin"));
    let model = if requested.exists() {
        requested
    } else {
        first_existing(
            &models,
            &["ggml-base.en.bin", "ggml-tiny.en.bin", "ggml-small.en.bin", "ggml-medium.en.bin"],
        )
        .ok_or("No whisper model found. Put ggml-base.en.bin in the project 'models' folder.")?
    };

    let binary = first_existing(&root.join("bin"), &["whisper-cli.exe", "main.exe", "whisper.exe"])
        .unwrap_or_else(|| PathBuf::from("whisper-cli")); // else rely on PATH

    Ok((model, binary))
}

#[tauri::command]
pub fn start_listening(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    model: Option<String>,
) -> Result<(), String> {
    if state.listening.load(Ordering::SeqCst) {
        return Ok(());
    }
    let kind = model.unwrap_or_else(|| "base".to_string());
    let (model, binary) = resolve_model_and_binary(&kind)?;
    state.listening.store(true, Ordering::SeqCst);
    let flag = state.listening.clone();
    let app2 = app.clone();
    std::thread::spawn(move || crate::audio::run_listen_loop(app2, flag, model, binary));
    Ok(())
}

#[tauri::command]
pub fn stop_listening(state: tauri::State<'_, AppState>) {
    state.listening.store(false, Ordering::SeqCst);
}

/// Start the LAN phone remote; returns the URL to open on a phone.
#[tauri::command]
pub fn start_remote(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<String, String> {
    crate::remote::start(app, state.remote_running.clone())
}
