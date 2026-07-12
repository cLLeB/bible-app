use crate::books::{book_after, book_before, book_by_osis};
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
    pub cursor: Mutex<Option<Cursor>>,   // currently-presented scripture position
    // Operator corrections: description signature -> chosen verse, so a repeated
    // paraphrase is ranked toward what the operator picked last time.
    pub learned: Mutex<std::collections::HashMap<String, (String, u16, u16)>>,
}

/// The scripture currently on screen, for fast verse/chapter navigation.
/// Navigation always resolves against the *active* translation, so switching
/// versions mid-presentation carries the position across.
#[derive(Clone)]
pub struct Cursor {
    pub book_osis: String,
    pub chapter: u16,
    pub verse: u16,
}

impl AppState {
    pub fn active_translation(&self) -> String {
        self.translation.lock().map(|t| t.clone()).unwrap_or_else(|_| "WEB".into())
    }
}

fn set_cursor(state: &AppState, book_osis: &str, chapter: u16, verse: u16) {
    if let Ok(mut c) = state.cursor.lock() {
        *c = Some(Cursor { book_osis: book_osis.to_string(), chapter, verse });
    }
}

/// Compute the target coordinates for a navigation step, crossing chapter and
/// book boundaries. Returns None at the ends of the canon.
fn compute_nav(
    db: &Db,
    tr: &str,
    cur: &Cursor,
    dir: &str,
) -> rusqlite::Result<Option<(String, u16, u16)>> {
    let res = match dir {
        "next-verse" => {
            let last = db.chapter_last_verse(tr, &cur.book_osis, cur.chapter)?.unwrap_or(0);
            if cur.verse < last {
                Some((cur.book_osis.clone(), cur.chapter, cur.verse + 1))
            } else if db.chapter_last_verse(tr, &cur.book_osis, cur.chapter + 1)?.is_some() {
                Some((cur.book_osis.clone(), cur.chapter + 1, 1))
            } else {
                book_after(&cur.book_osis).map(|b| (b.osis.to_string(), 1, 1))
            }
        }
        "prev-verse" => {
            if cur.verse > 1 {
                Some((cur.book_osis.clone(), cur.chapter, cur.verse - 1))
            } else if cur.chapter > 1 {
                let lv = db.chapter_last_verse(tr, &cur.book_osis, cur.chapter - 1)?.unwrap_or(1);
                Some((cur.book_osis.clone(), cur.chapter - 1, lv))
            } else if let Some(b) = book_before(&cur.book_osis) {
                let lc = db.book_last_chapter(tr, b.osis)?.unwrap_or(1);
                let lv = db.chapter_last_verse(tr, b.osis, lc)?.unwrap_or(1);
                Some((b.osis.to_string(), lc, lv))
            } else {
                None
            }
        }
        "next-chapter" => {
            if db.chapter_last_verse(tr, &cur.book_osis, cur.chapter + 1)?.is_some() {
                Some((cur.book_osis.clone(), cur.chapter + 1, 1))
            } else {
                book_after(&cur.book_osis).map(|b| (b.osis.to_string(), 1, 1))
            }
        }
        "prev-chapter" => {
            if cur.chapter > 1 {
                Some((cur.book_osis.clone(), cur.chapter - 1, 1))
            } else if let Some(b) = book_before(&cur.book_osis) {
                let lc = db.book_last_chapter(tr, b.osis)?.unwrap_or(1);
                Some((b.osis.to_string(), lc, 1))
            } else {
                None
            }
        }
        _ => None,
    };
    Ok(res)
}

/// Present a verse by exact coordinates: project it and set the cursor.
pub(crate) fn present_coords_handle(
    app: &tauri::AppHandle,
    book_osis: &str,
    chapter: u16,
    verse: u16,
) -> Option<VersePayload> {
    let state = app.state::<AppState>();
    let tr = state.active_translation();
    let rec = {
        let db = state.db.lock().ok()?;
        db.verse_at(&tr, book_osis, chapter, verse).ok().flatten()
    }?;
    let payload = build_payload(rec);
    set_cursor(&state, &payload.book_osis, payload.chapter, payload.verse);
    let caption = format!("{} · {}", payload.reference, payload.translation);
    let _ = project_via_handle(app, ProjectionState::Verse { text: payload.text.clone(), caption });
    Some(payload)
}

/// Move the presented scripture in a direction; returns the new verse (or None
/// at a boundary / when nothing is presented yet).
pub(crate) fn navigate_handle(app: &tauri::AppHandle, dir: &str) -> Option<VersePayload> {
    let state = app.state::<AppState>();
    let cur = state.cursor.lock().ok().and_then(|c| c.clone())?;
    let tr = state.active_translation();
    let target = {
        let db = state.db.lock().ok()?;
        compute_nav(&db, &tr, &cur, dir).ok().flatten()
    };
    let (osis, ch, v) = target?;
    present_coords_handle(app, &osis, ch, v)
}

#[tauri::command]
pub fn present_coords(
    app: tauri::AppHandle,
    book_osis: String,
    chapter: u16,
    verse: u16,
) -> Result<VersePayload, String> {
    present_coords_handle(&app, &book_osis, chapter, verse)
        .ok_or_else(|| "Verse not found".to_string())
}

#[tauri::command]
pub fn navigate(app: tauri::AppHandle, dir: String) -> Option<VersePayload> {
    navigate_handle(&app, &dir)
}

pub(crate) fn build_payload(rec: crate::db::VerseRecord) -> VersePayload {
    let book_name = book_by_osis(&rec.book_osis)
        .map(|b| b.name.to_string())
        .unwrap_or_else(|| rec.book_osis.clone());
    let reference = format!("{} {}:{}", book_name, rec.chapter, rec.verse);
    VersePayload {
        reference,
        book: book_name,
        book_osis: rec.book_osis,
        chapter: rec.chapter,
        verse: rec.verse,
        text: rec.text,
        translation: rec.translation,
    }
}

/// Build a payload for a verse range ("John 3:16-18"): the combined text with a
/// range caption. `verse` holds the start. Returns None if the range is empty.
pub(crate) fn build_range_payload(
    db: &crate::db::Db,
    translation: &str,
    book_osis: &str,
    chapter: u16,
    start: u16,
    end: u16,
) -> Option<VersePayload> {
    let text = db.find_verse_range(translation, book_osis, chapter, start, end).ok()??;
    let book_name = book_by_osis(book_osis)
        .map(|b| b.name.to_string())
        .unwrap_or_else(|| book_osis.to_string());
    Some(VersePayload {
        reference: format!("{book_name} {chapter}:{start}-{end}"),
        book: book_name,
        book_osis: book_osis.to_string(),
        chapter,
        verse: start,
        text,
        translation: translation.to_string(),
    })
}

/// Detect a translation named in text ("in ASV", "the King James", "world english").
/// Returns the CODE if a known name/abbrev appears (regardless of install state).
fn parse_translation_code(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    let names: &[(&str, &str)] = &[
        ("king james", "KJV"),
        ("american standard", "ASV"),
        ("world english", "WEB"),
        ("young's literal", "YLT"),
        ("youngs literal", "YLT"),
        ("basic english", "BBE"),
        ("darby", "DARBY"),
    ];
    for (n, c) in names {
        if lower.contains(n) {
            return Some(c);
        }
    }
    let tokens: Vec<String> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    let abbr: &[(&str, &str)] = &[
        ("kjv", "KJV"), ("web", "WEB"), ("asv", "ASV"), ("ylt", "YLT"), ("bbe", "BBE"),
    ];
    for (a, c) in abbr {
        if tokens.iter().any(|t| t == a) {
            return Some(c);
        }
    }
    None
}

/// Remove a translation name/abbrev (and adjacent filler words) from a typed
/// query so the reference parser gets a clean "Book chapter:verse".
fn strip_translation_phrase(query: &str) -> String {
    let mut q = query.to_string();
    let pats = [
        "king james version", "king james", "american standard version", "american standard",
        "world english bible", "world english", "young's literal translation", "young's literal",
        "youngs literal", "bible in basic english", "basic english", "darby translation", "darby",
        "kjv", "web", "asv", "ylt", "bbe",
    ];
    for pat in pats {
        while let Some(pos) = q.to_lowercase().find(pat) {
            q.replace_range(pos..pos + pat.len(), " ");
        }
    }
    q.split_whitespace()
        .filter(|w| !["in", "from", "the", "version", "translation"].contains(&w.to_lowercase().as_str()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// If `text` names an installed translation, switch the active translation to it
/// and return it; otherwise return the current active translation (fallback).
pub(crate) fn resolve_translation(state: &AppState, text: &str) -> String {
    if let Some(code) = parse_translation_code(text) {
        let exists = state
            .db
            .lock()
            .ok()
            .and_then(|db| db.has_translation(code).ok())
            .unwrap_or(false);
        if exists {
            if let Ok(mut t) = state.translation.lock() {
                *t = code.to_string();
            }
            return code.to_string();
        }
    }
    state.active_translation()
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
    let tr = resolve_translation(state, query);
    let cleaned = strip_translation_phrase(query);
    let (base_query, end) = extract_range(&cleaned);
    // Plain reference first; then fall back to deep knowledge (descriptive book
    // names, famous stories) so "the prodigal son" or "last book of the OT" work.
    let parsed = match parse_reference(&base_query) {
        Some(p) => p,
        None => crate::detect::detect_with_context(&base_query, &mut crate::detect::RefContext::default())
            .into_iter()
            .next()
            .map(|d| d.reference)
            .ok_or_else(|| format!("Could not find '{query}'"))?,
    };
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
                    book_osis: parsed.book_osis.clone(),
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
    app: tauri::AppHandle,
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<VersePayload, String> {
    let result = do_lookup(&state, &query)?;
    // Announce the active translation so the picker reflects any spoken/typed switch.
    let _ = app.emit("translation-changed", &result.translation);
    Ok(result)
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

/// Split a passage into readable projection-sized slides at word boundaries.
#[tauri::command]
pub fn chunk_passage(text: String, max_chars: usize) -> Vec<String> {
    let cap = if max_chars == 0 { 220 } else { max_chars };
    crate::slides::chunk_text(&text, cap)
}

/// Remember that, for the paraphrase in `transcript`, the operator chose this
/// verse — so the next matching description ranks it first.
#[tauri::command]
pub fn record_choice(
    transcript: String,
    book_osis: String,
    chapter: u16,
    verse: u16,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let sig = crate::resolution::signature(&transcript);
    if sig.is_empty() {
        return Ok(());
    }
    let mut learned = state.learned.lock().map_err(|e| e.to_string())?;
    learned.insert(sig, (book_osis, chapter, verse));
    Ok(())
}

/// Cross-references for a verse ("related verses"), resolved in the active
/// translation. Skips any that aren't present in the installed text.
#[tauri::command]
pub fn related_verses(
    book_osis: String,
    chapter: u16,
    verse: u16,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<VersePayload>, String> {
    let tr = state.active_translation();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (osis, ch, v) in crate::knowledge::related_verses(&book_osis, chapter, verse) {
        if let Ok(Some(rec)) = db.verse_at(&tr, &osis, ch, v) {
            out.push(build_payload(rec));
        }
    }
    Ok(out)
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
    if db.is_built_in(song_id).map_err(|e| e.to_string())? {
        return Err("Bundled hymns can't be edited".into());
    }
    db.update_song(song_id, &title, author.as_deref(), &lyrics)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_song(song_id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    if db.is_built_in(song_id).map_err(|e| e.to_string())? {
        return Err("Bundled hymns can't be deleted".into());
    }
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
    set_cursor(&state, &payload.book_osis, payload.chapter, payload.verse);
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
        // Projection shows just the title — no slide numbers on the wall.
        (slide.text.clone(), title)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_strips_spoken_translation() {
        assert_eq!(parse_translation_code("John 3:16 in ASV"), Some("ASV"));
        assert_eq!(parse_translation_code("give me the King James Romans 8"), Some("KJV"));
        assert_eq!(parse_translation_code("world english bible please"), Some("WEB"));
        assert_eq!(parse_translation_code("John 3:16"), None);

        assert_eq!(strip_translation_phrase("John 3:16 in ASV").trim(), "John 3:16");
        assert_eq!(strip_translation_phrase("the King James John 3:16").trim(), "John 3:16");
        assert_eq!(strip_translation_phrase("Romans 8:28").trim(), "Romans 8:28");
    }
}
