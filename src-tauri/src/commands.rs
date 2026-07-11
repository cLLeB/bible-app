use crate::books::book_by_osis;
use crate::db::Db;
use crate::events::VersePayload;
use crate::reference::parse_reference;
use std::sync::Mutex;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub struct AppState {
    pub db: Mutex<Db>,
    pub translation: String, // active translation code, e.g. "WEB"
}

fn build_payload(rec: crate::db::VerseRecord) -> VersePayload {
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

#[tauri::command]
pub fn lookup_reference(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<VersePayload, String> {
    let parsed = parse_reference(&query).ok_or_else(|| format!("Could not parse '{query}'"))?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let rec = db
        .find_verse(&state.translation, &parsed)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Verse not found: '{query}'"))?;
    Ok(build_payload(rec))
}

fn ensure_projection_window(app: &tauri::AppHandle) -> Result<(), String> {
    if app.get_webview_window("projection").is_some() {
        return Ok(());
    }
    let win = WebviewWindowBuilder::new(app, "projection", WebviewUrl::App("projection.html".into()))
        .title("Projection")
        .decorations(false)
        .build()
        .map_err(|e| e.to_string())?;

    // Move to the second monitor if present, else stay on primary.
    if let Ok(monitors) = win.available_monitors() {
        if let Some(second) = monitors.get(1) {
            let pos = second.position();
            win.set_position(tauri::PhysicalPosition { x: pos.x, y: pos.y })
                .map_err(|e| e.to_string())?;
        }
    }
    win.set_fullscreen(true).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn project_verse(app: tauri::AppHandle, payload: VersePayload) -> Result<(), String> {
    ensure_projection_window(&app)?;
    app.emit_to("projection", "set-projection", Some(payload))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn blank_projection(app: tauri::AppHandle) -> Result<(), String> {
    ensure_projection_window(&app)?;
    app.emit_to("projection", "set-projection", Option::<VersePayload>::None)
        .map_err(|e| e.to_string())
}
