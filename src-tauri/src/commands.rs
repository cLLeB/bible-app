use crate::books::{book_after, book_before, book_by_osis};
use crate::db::{Db, SongSummary};
use crate::events::{Alert, ProjectionSettings, ProjectionState, StageInfo, StageSlot, VersePayload};
use crate::reference::parse_reference;
use crate::themes::{self, Theme};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

pub struct AppState {
    pub db: Mutex<Db>,
    pub translation: Mutex<String>, // active translation code, e.g. "WEB"
    pub current: Mutex<ProjectionState>, // what the projection should show
    pub settings: Mutex<ProjectionSettings>, // display appearance
    pub stage: Mutex<StageInfo>,         // what the stage/confidence monitor shows
    pub alert: Mutex<crate::events::Alert>, // lower-third alert over the congregation screen
    // Sound under the service. Separate from `current` on purpose: music plays
    // *while* a verse or song holds the screen (see events::AudioState).
    pub audio: Mutex<crate::events::AudioState>,
    // First-run seeding runs on a background thread (it holds the db lock for
    // as long as it takes), so the UI can tell "still installing" from "ready".
    pub ready: Arc<AtomicBool>,
    pub listening: Arc<AtomicBool>,      // mic listen loop active?
    pub recording: Arc<AtomicBool>,      // record this service for learning?
    pub remote_running: Arc<AtomicBool>, // LAN remote server started?
    pub slideshow: Arc<AtomicBool>,      // announcements loop advancing?
    // Set by the projection window when the live video reaches its end, so the
    // loop can move on at the clip's own length instead of an image's timer.
    pub video_ended: Arc<AtomicBool>,
    pub cursor: Mutex<Option<Cursor>>,   // currently-presented scripture position
    // Operator corrections: description signature -> chosen verse, so a repeated
    // paraphrase is ranked toward what the operator picked last time.
    pub learned: Mutex<std::collections::HashMap<String, (String, u16, u16)>>,
    // Moments captured during a recorded service, written to the session on stop.
    pub moments: Mutex<Vec<crate::sessions::Moment>>,
    // A background learning pass is running. Clearing it stops the pass.
    pub learning: Arc<AtomicBool>,
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

/// Who drove a presentation.
///
/// The console applies its own changes as it makes them, so it only needs to
/// hear about the ones it did not cause. Naming the source is what lets it tell
/// the difference: mirroring a console-driven change back would hand scripture
/// the keyboard in the middle of a service order that already owns it.
pub(crate) const BY_CONSOLE: &str = "console";
pub(crate) const BY_REMOTE: &str = "remote";
pub(crate) const BY_VOICE: &str = "voice";

/// Record a verse as the presented one: move the cursor and tell the desktop.
///
/// Whoever drove the change (laptop, voice, or the phone in the operator's
/// pocket) the cursor is what next/previous steps from and "Presenting" is what
/// the operator reads. Both have to follow the wall, or the next tap moves from
/// a verse nobody is looking at.
pub(crate) fn mark_presented(app: &tauri::AppHandle, payload: &VersePayload, source: &str) {
    let state = app.state::<AppState>();
    set_cursor(&state, &payload.book_osis, payload.chapter, payload.verse);
    let _ = app.emit(
        "presenting-changed",
        serde_json::json!({ "verse": payload, "source": source }),
    );
}

/// Present a verse by exact coordinates: project it and set the cursor.
pub(crate) fn present_coords_handle(
    app: &tauri::AppHandle,
    book_osis: &str,
    chapter: u16,
    verse: u16,
    source: &str,
) -> Option<VersePayload> {
    let state = app.state::<AppState>();
    let tr = state.active_translation();
    let rec = {
        let db = state.db.lock().ok()?;
        db.verse_at(&tr, book_osis, chapter, verse).ok().flatten()
    }?;
    let payload = build_payload(rec);
    let caption = format!("{} · {}", payload.reference, payload.translation);
    let _ = project_via_handle(app, ProjectionState::Verse { text: payload.text.clone(), caption });
    mark_presented(app, &payload, source);
    Some(payload)
}

/// Present a spoken or typed reference: look it up, project it, and record it.
/// The path the LAN remote takes, so a verse sent from the phone lands exactly
/// where one presented at the laptop does.
pub(crate) fn present_reference_handle(
    app: &tauri::AppHandle,
    query: &str,
) -> Result<VersePayload, String> {
    let payload = {
        let state = app.state::<AppState>();
        do_lookup(&state, query)?
    };
    let caption = format!("{} · {}", payload.reference, payload.translation);
    project_via_handle(app, ProjectionState::Verse { text: payload.text.clone(), caption })?;
    mark_presented(app, &payload, BY_REMOTE);
    Ok(payload)
}

/// Move the presented scripture in a direction; returns the new verse (or None
/// at a boundary / when nothing is presented yet).
pub(crate) fn navigate_handle(
    app: &tauri::AppHandle,
    dir: &str,
    source: &str,
) -> Option<VersePayload> {
    let state = app.state::<AppState>();
    let cur = state.cursor.lock().ok().and_then(|c| c.clone())?;
    let tr = state.active_translation();
    let target = {
        let db = state.db.lock().ok()?;
        compute_nav(&db, &tr, &cur, dir).ok().flatten()
    };
    let (osis, ch, v) = target?;
    present_coords_handle(app, &osis, ch, v, source)
}

#[tauri::command]
pub fn present_coords(
    app: tauri::AppHandle,
    book_osis: String,
    chapter: u16,
    verse: u16,
) -> Result<VersePayload, String> {
    present_coords_handle(&app, &book_osis, chapter, verse, BY_CONSOLE)
        .ok_or_else(|| "Verse not found".to_string())
}

#[tauri::command]
pub fn navigate(app: tauri::AppHandle, dir: String) -> Option<VersePayload> {
    navigate_handle(&app, &dir, BY_CONSOLE)
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

/// Spoken/typed full-name phrases → translation code, covering every
/// translation the app supports. Matched by substring with LONGEST-match
/// precedence, so "new king james" resolves to NKJV (not KJV) and "new american
/// standard" to NASB (not ASV). Kept in sync with `translations.rs`.
fn translation_phrases() -> &'static [(&'static str, &'static str, bool)] {
    // (phrase, code, word_safe). word_safe = false marks a phrase that is (or
    // hinges on) an everyday English word ("amplified", "the message") — matched
    // only when a scripture reference is present in the utterance, so ordinary
    // speech ("the amplified guitar") never switches translation.
    &[
        // Public domain / free
        ("king james version", "KJV", true), ("king james", "KJV", true), ("authorized version", "KJV", true),
        ("american standard version", "ASV", true), ("american standard", "ASV", true),
        ("world english bible", "WEB", true), ("world english", "WEB", true),
        ("young's literal translation", "YLT", true), ("young's literal", "YLT", true), ("youngs literal", "YLT", true),
        ("bible in basic english", "BBE", true), ("basic english", "BBE", true),
        ("darby translation", "DARBY", true), ("darby bible", "DARBY", true), ("darby", "DARBY", true),
        ("berean standard bible", "BSB", true), ("berean standard", "BSB", true), ("berean bible", "BSB", true), ("berean", "BSB", true),
        ("geneva bible", "GNV", true), ("geneva", "GNV", true),
        ("douay rheims", "DRB", true), ("douay-rheims", "DRB", true), ("douay", "DRB", true), ("rheims", "DRB", true),
        ("webster's bible", "WBT", true), ("webster bible", "WBT", true), ("webster", "WBT", true),
        ("brenton septuagint", "LXXE", true), ("septuagint", "LXXE", true),
        // Copyrighted (recognized so the operator can speak/type them; only
        // switched to when actually installed — Personal-tier builds)
        ("new international version", "NIV", true), ("new international", "NIV", true),
        ("new living translation", "NLT", true), ("new living", "NLT", true),
        ("english standard version", "ESV", true), ("english standard", "ESV", true),
        ("new king james version", "NKJV", true), ("new king james", "NKJV", true),
        ("new american standard bible", "NASB", true), ("new american standard", "NASB", true),
        ("christian standard bible", "CSB17", true), ("christian standard", "CSB17", true),
        ("amplified bible", "AMP", true), ("amplified version", "AMP", false), ("amplified", "AMP", false),
        ("the message bible", "MSG", true), ("message bible", "MSG", true),
        ("new english translation", "NET", true), ("net bible", "NET", true),
        // More popular translations (Personal tier)
        ("good news bible", "GNT", true), ("good news translation", "GNTD", true), ("good news", "GNT", false),
        ("new revised standard version", "NRSVCE", true), ("new revised standard", "NRSVCE", true),
        ("revised standard version", "RSV", true), ("revised standard", "RSV", true),
        ("common english bible", "CEB", true), ("common english", "CEB", true),
        ("contemporary english version", "CEVD", true), ("contemporary english", "CEVD", true),
        ("complete jewish bible", "CJB", true), ("complete jewish", "CJB", true),
        ("tree of life version", "TLV", true), ("tree of life", "TLV", false),
        ("legacy standard bible", "LSB", true), ("legacy standard", "LSB", true),
        ("modern english version", "MEV", true), ("modern english", "MEV", true),
        ("international standard version", "ISV", true), ("international standard", "ISV", true),
        ("easy to read version", "ERV", true), ("easy-to-read version", "ERV", true), ("easy to read", "ERV", true),
        ("new life version", "NLV", true), ("new life", "NLV", false),
        ("new american bible", "NABRE", true),
        ("literal standard version", "LSV", true), ("literal standard", "LSV", true),
    ]
}

/// Distinctive abbreviations → (code, word_safe). `word_safe = true` may match a
/// lone word token anywhere; `false` (net, amp, msg — everyday English words)
/// matches ONLY when spelled letter-by-letter or when a scripture reference is
/// present, so a sermon line like "cast the net" never switches translation.
fn translation_abbrevs() -> &'static [(&'static str, &'static str, bool)] {
    &[
        ("kjv", "KJV", true), ("asv", "ASV", true), ("web", "WEB", true), ("ylt", "YLT", true),
        ("bbe", "BBE", true), ("darby", "DARBY", true), ("bsb", "BSB", true), ("gnv", "GNV", true),
        ("drb", "DRB", true), ("wbt", "WBT", true), ("lxxe", "LXXE", true), ("niv", "NIV", true),
        ("nlt", "NLT", true), ("esv", "ESV", true), ("nkjv", "NKJV", true), ("nasb", "NASB", true),
        ("csb", "CSB17", true), ("csb17", "CSB17", true),
        ("gnt", "GNT", true), ("gnb", "GNT", true), ("gntd", "GNTD", true), ("rsv", "RSV", true),
        ("nrsv", "NRSVCE", true), ("nrsvce", "NRSVCE", true), ("ceb", "CEB", true),
        ("cev", "CEVD", true), ("cevd", "CEVD", true), ("cjb", "CJB", true), ("tlv", "TLV", true),
        ("lsb", "LSB", true), ("mev", "MEV", true), ("isv", "ISV", true), ("erv", "ERV", true),
        ("nlv", "NLV", true), ("nabre", "NABRE", true), ("lsv", "LSV", true),
        ("net", "NET", false), ("amp", "AMP", false), ("msg", "MSG", false),
    ]
}

fn abbrev_core(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_ascii_alphanumeric()).to_lowercase()
}

fn tokens_lower(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

/// Joined strings from runs of ≥2 consecutive single-letter tokens — a spelled
/// abbreviation. Pastors say "N-I-V"; whisper renders it "N I V" / "N.I.V." →
/// tokens ["n","i","v"] → "niv". Numbers break a run (so "3 16 N I V" is fine).
fn spelled_abbrevs(tokens: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let is_letter = |t: &String| t.len() == 1 && t.chars().all(|c| c.is_ascii_alphabetic());
        if is_letter(&tokens[i]) {
            let mut j = i;
            let mut s = String::new();
            while j < tokens.len() && is_letter(&tokens[j]) {
                s.push_str(&tokens[j]);
                j += 1;
            }
            if j - i >= 2 {
                out.push(s);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Detect a translation named in text ("in ASV", "the New King James", "N I V",
/// "amplified"). Returns the CODE if a known name/abbrev appears (regardless of
/// install state). `reference_present` relaxes the word-safety guard so a typed
/// "John 3:16 NET" resolves while ordinary prose does not.
fn parse_translation_code(text: &str, reference_present: bool) -> Option<&'static str> {
    let lower = text.to_lowercase();
    // Full names: longest matching phrase wins (specific beats general). A
    // word-like phrase ("amplified") only counts when a scripture is present.
    let mut best_safe: Option<(usize, &str)> = None;
    let mut best_word: Option<(usize, &str)> = None;
    for (phrase, code, word_safe) in translation_phrases() {
        if lower.contains(phrase) {
            let slot = if *word_safe { &mut best_safe } else { &mut best_word };
            if slot.is_none_or(|(len, _)| phrase.len() > len) {
                *slot = Some((phrase.len(), code));
            }
        }
    }
    let word_ok = if reference_present { best_word } else { None };
    let chosen = match (best_safe, word_ok) {
        (Some(s), Some(w)) => Some(if w.0 > s.0 { w.1 } else { s.1 }),
        (Some(s), None) => Some(s.1),
        (None, Some(w)) => Some(w.1),
        (None, None) => None,
    };
    if let Some(code) = chosen {
        return Some(code);
    }
    // Abbreviations: spelled letter-runs match unconditionally (unambiguous);
    // lone word tokens match if word-safe or if a scripture reference is near.
    let tokens = tokens_lower(text);
    let spelled = spelled_abbrevs(&tokens);
    for (ab, code, word_safe) in translation_abbrevs() {
        if spelled.iter().any(|s| s == ab) {
            return Some(code);
        }
        if (*word_safe || reference_present) && tokens.iter().any(|t| t == ab) {
            return Some(code);
        }
    }
    None
}

/// Remove a translation name/abbrev/spelled-abbrev (and filler words) from a
/// typed query so the reference parser gets a clean "Book chapter:verse".
fn strip_translation_phrase(query: &str) -> String {
    let mut q = query.to_string();
    // Full-name phrases, longest first so multi-word names strip completely.
    let mut phrases: Vec<&str> = translation_phrases().iter().map(|(p, _, _)| *p).collect();
    phrases.sort_by_key(|p| std::cmp::Reverse(p.len()));
    for pat in phrases {
        while let Some(pos) = q.to_lowercase().find(pat) {
            q.replace_range(pos..pos + pat.len(), " ");
        }
    }
    let known: std::collections::HashSet<&str> =
        translation_abbrevs().iter().map(|(a, _, _)| *a).collect();
    let filler = ["in", "from", "the", "version", "translation", "bible"];
    let words: Vec<&str> = q.split_whitespace().collect();
    let cores: Vec<String> = words.iter().map(|w| abbrev_core(w)).collect();
    let mut drop = vec![false; words.len()];
    // Drop runs of single letters that spell a known abbreviation ("n i v").
    let mut i = 0;
    while i < cores.len() {
        if cores[i].len() == 1 {
            let mut j = i;
            let mut s = String::new();
            while j < cores.len() && cores[j].len() == 1 {
                s.push_str(&cores[j]);
                j += 1;
            }
            if j - i >= 2 && known.contains(s.as_str()) {
                (i..j).for_each(|k| drop[k] = true);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    // Drop lone abbreviations and filler words.
    for (k, c) in cores.iter().enumerate() {
        if known.contains(c.as_str()) || filler.contains(&c.as_str()) {
            drop[k] = true;
        }
    }
    words
        .iter()
        .zip(drop)
        .filter(|(_, d)| !*d)
        .map(|(w, _)| *w)
        .collect::<Vec<_>>()
        .join(" ")
}

/// If `text` names an installed translation, switch the active translation to it
/// and return it; otherwise return the current active translation (fallback).
pub(crate) fn resolve_translation(state: &AppState, text: &str) -> String {
    // "Around a scripture": a reference in the same utterance lets us trust even
    // the everyday-word abbreviations (NET/AMP/MSG) as a translation request.
    let reference_present = !crate::detect::detect_references(text).is_empty();
    if let Some(code) = parse_translation_code(text, reference_present) {
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

/// This build's flavor: license tier + which whisper models it ships.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlavorInfo {
    pub tier: String,
    pub models: Vec<String>,
    pub default_model: String,
}

#[tauri::command]
pub fn app_flavor() -> FlavorInfo {
    FlavorInfo {
        tier: crate::flavor::tier_name().to_string(),
        models: crate::flavor::models().into_iter().map(String::from).collect(),
        default_model: crate::flavor::default_model().to_string(),
    }
}

/// The downloadable translation catalog with each entry's installed state.
/// Includes copyrighted translations only in a Personal-tier build.
#[tauri::command]
pub fn translation_catalog(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::translations::CatalogEntry>, String> {
    let installed: Vec<String> = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.list_translations().map_err(|e| e.to_string())?.into_iter().map(|(c, _)| c).collect()
    };
    Ok(crate::translations::catalog(crate::flavor::is_personal(), &installed))
}

/// Download a translation and store it for offline use. Returns the number of
/// verses installed. Refuses codes not available in this build's tier.
#[tauri::command]
pub fn download_translation(
    app: tauri::AppHandle,
    code: String,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    // Network fetch happens without holding the DB lock.
    let canonical = crate::translations::fetch_canonical(&code, crate::flavor::is_personal())?;
    let n = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.seed_from_json(&canonical).map_err(|e| e.to_string())?
    };
    let _ = app.emit("translation-installed", &code);
    Ok(n)
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

/// Has first-run seeding finished? Deliberately touches no locks, so the UI can
/// poll it while the seeding thread still holds the db.
#[tauri::command]
pub fn library_ready(state: tauri::State<'_, AppState>) -> bool {
    state.ready.load(Ordering::Relaxed)
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
/// Every screen the OS is currently offering.
pub(crate) fn displays_now(app: &tauri::AppHandle) -> Vec<crate::displays::DisplayInfo> {
    let Some(win) = app.get_webview_window("projection") else { return Vec::new() };
    let primary = win.primary_monitor().ok().flatten().and_then(|m| m.name().cloned());
    win.available_monitors()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(i, m)| {
            let name = m.name().cloned().unwrap_or_else(|| format!("Display {}", i + 1));
            let size = m.size();
            let pos = m.position();
            crate::displays::DisplayInfo {
                primary: Some(&name) == primary.as_ref(),
                name,
                x: pos.x,
                y: pos.y,
                width: size.width,
                height: size.height,
            }
        })
        .collect()
}

const OUTPUT_DISPLAY_KEY: &str = "projection:display";

/// The screen the operator picked, or None for automatic.
fn preferred_display(app: &tauri::AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    let db = state.db.lock().ok()?;
    db.get_setting(OUTPUT_DISPLAY_KEY).filter(|s| !s.trim().is_empty())
}

/// The projection window is declared (hidden) in tauri.conf.json, so it loads
/// index.html at startup exactly like the main window. Here we place it on the
/// right screen and reveal it.
///
/// Two rules the ordinary church setup depends on. It goes to the screen that
/// is *not* the operator's, chosen by that property rather than by enumeration
/// order. And it is never focused: stealing the caret out of the search box on
/// every cue, and dragging the window onto whatever virtual desktop the
/// operator is using, is what output windows must not do.
fn ensure_projection_window(app: &tauri::AppHandle) -> Result<(), String> {
    place_projection_window(app)
}

/// Place and reveal the output window. Separate from the projection path so the
/// display watcher can re-place a window that is already showing without
/// pretending something new is being projected.
pub(crate) fn place_projection_window(app: &tauri::AppHandle) -> Result<(), String> {
    let win = app
        .get_webview_window("projection")
        .ok_or_else(|| "projection window not found".to_string())?;

    let displays = displays_now(app);
    let preferred = preferred_display(app);
    let want = match preferred.as_deref() {
        Some(name) => crate::displays::Choice::Named(name),
        None => crate::displays::Choice::Automatic,
    };
    let target = crate::displays::choose(&displays, want);

    if let Some(display) = target {
        win.set_fullscreen(false).ok();
        win.set_position(tauri::PhysicalPosition { x: display.x, y: display.y })
            .map_err(|e| e.to_string())?;
        if crate::displays::should_fill(Some(display)) {
            win.set_fullscreen(true).map_err(|e| e.to_string())?;
        } else {
            // The operator's own screen: a window they can move and see past,
            // not a takeover of the console they are working in.
            win.set_size(tauri::PhysicalSize { width: 960, height: 540 })
                .map_err(|e| e.to_string())?;
        }
    }

    win.show().map_err(|e| e.to_string())?;
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
        // Log usage once per day for CCLI reporting (ignore logging errors).
        let _ = db.log_song_usage(song_id);
        (slide.text.clone(), title)
    };
    project(&app, &state, ProjectionState::Song { text, caption })
}

/// The CCLI song-usage report: which songs were shown, how many service days,
/// and when last used. Most-used first.
#[tauri::command]
pub fn song_usage_report(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::db::UsageRow>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.song_usage_report().map_err(|e| e.to_string())
}

/// Project the same verse in two translations side by side. Primary is the
/// active translation; `secondary` is any installed translation code. Missing
/// secondary text projects an empty column rather than failing.
///
/// Shared by the console and the LAN remote, so a comparison started on the
/// phone leaves the cursor and "Presenting" exactly where one started at the
/// laptop does.
pub(crate) fn present_parallel_handle(
    app: &tauri::AppHandle,
    book_osis: &str,
    chapter: u16,
    verse: u16,
    secondary: &str,
    source: &str,
) -> Result<VersePayload, String> {
    let (payload, second) = {
        let state = app.state::<AppState>();
        let tr = state.active_translation();
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let primary = db
            .verse_at(&tr, book_osis, chapter, verse)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Verse not found".to_string())?;
        let second = db.verse_at(secondary, book_osis, chapter, verse).map_err(|e| e.to_string())?;
        (build_payload(primary), second)
    };
    let st = ProjectionState::Parallel {
        primary_text: payload.text.clone(),
        primary_code: payload.translation.clone(),
        secondary_text: second.map(|r| r.text).unwrap_or_default(),
        secondary_code: secondary.to_string(),
        caption: payload.reference.clone(),
    };
    project_via_handle(app, st)?;
    mark_presented(app, &payload, source);
    Ok(payload)
}

/// Compare by reference rather than coordinates. An empty reference means
/// "whatever is on screen", so the phone's Both works straight after a nav step
/// without retyping where the preacher already is.
pub(crate) fn present_parallel_ref_handle(
    app: &tauri::AppHandle,
    secondary: &str,
    reference: &str,
) -> Result<VersePayload, String> {
    let state = app.state::<AppState>();
    let (book_osis, chapter, verse) = if reference.is_empty() {
        let cur = state
            .cursor
            .lock()
            .ok()
            .and_then(|c| c.clone())
            .ok_or_else(|| "Nothing is on screen yet. Project a verse first.".to_string())?;
        (cur.book_osis, cur.chapter, cur.verse)
    } else {
        let v = do_lookup(&state, reference)?;
        (v.book_osis, v.chapter, v.verse)
    };
    present_parallel_handle(app, &book_osis, chapter, verse, secondary, BY_REMOTE)
}

#[tauri::command]
pub fn project_parallel(
    app: tauri::AppHandle,
    book_osis: String,
    chapter: u16,
    verse: u16,
    secondary: String,
) -> Result<VersePayload, String> {
    present_parallel_handle(&app, &book_osis, chapter, verse, &secondary, BY_CONSOLE)
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

/// Every screen, with the one output is currently going to marked. Shown to the
/// operator so choosing a TV is a decision they make rather than a guess the
/// app makes silently.
#[tauri::command]
pub fn list_displays(app: tauri::AppHandle) -> (Vec<crate::displays::DisplayInfo>, String) {
    let displays = displays_now(&app);
    (displays, preferred_display(&app).unwrap_or_default())
}

/// Send output to a named screen. An empty name means automatic: whichever
/// screen is not the operator's.
#[tauri::command]
pub fn set_output_display(app: tauri::AppHandle, name: String) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.set_setting(OUTPUT_DISPLAY_KEY, name.trim()).map_err(|e| e.to_string())?;
    }
    // Move immediately: the operator should see the change land, not discover
    // it at the next cue.
    ensure_projection_window(&app)
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

/// Store the resolved settings in shared state and push them to the projection
/// window. The single seam every appearance change flows through.
fn apply_settings(
    app: &tauri::AppHandle,
    state: &AppState,
    settings: ProjectionSettings,
) -> Result<(), String> {
    if let Ok(mut s) = state.settings.lock() {
        *s = settings.clone();
    }
    app.emit_to("projection", "set-settings", settings)
        .map_err(|e| e.to_string())
}

/// Every theme the operator can choose from (built-ins first, then custom).
#[tauri::command]
pub fn list_themes(state: tauri::State<'_, AppState>) -> Result<Vec<Theme>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    Ok(themes::all_themes(&db))
}

/// Make a theme the active look: persist the choice, resolve it, and project it.
#[tauri::command]
pub fn set_active_theme(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<ProjectionSettings, String> {
    let theme = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        themes::set_active_theme_id(&db, &id).map_err(|e| e.to_string())?;
        themes::theme_by_id(&db, &id)
    };
    let font_scale = state.settings.lock().map(|s| s.font_scale).unwrap_or(1.0);
    let settings = ProjectionSettings { font_scale, theme };
    apply_settings(&app, &state, settings.clone())?;
    Ok(settings)
}

/// Adjust the global font multiplier; persisted and applied live.
#[tauri::command]
pub fn set_font_scale(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    scale: f32,
) -> Result<(), String> {
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        themes::set_font_scale(&db, scale).map_err(|e| e.to_string())?;
    }
    let theme = state.settings.lock().map(|s| s.theme.clone()).unwrap_or_else(|_| themes::default_theme());
    apply_settings(&app, &state, ProjectionSettings { font_scale: scale, theme })
}

/// Create or update a custom theme. Editing the live theme re-projects it at once.
#[tauri::command]
pub fn save_theme(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    theme: Theme,
) -> Result<(), String> {
    let is_active = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        themes::save_custom(&db, &theme).map_err(|e| e.to_string())?;
        themes::active_theme_id(&db) == theme.id
    };
    if is_active {
        let font_scale = state.settings.lock().map(|s| s.font_scale).unwrap_or(1.0);
        let stored = Theme { built_in: false, ..theme };
        apply_settings(&app, &state, ProjectionSettings { font_scale, theme: stored })?;
    }
    Ok(())
}

/// Delete a custom theme. If it was live, fall back to the default look.
#[tauri::command]
pub fn delete_theme(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let fell_back = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        themes::delete_custom(&db, &id).map_err(|e| e.to_string())?;
        if themes::active_theme_id(&db) == id {
            themes::set_active_theme_id(&db, themes::default_theme_id()).map_err(|e| e.to_string())?;
            true
        } else {
            false
        }
    };
    if fell_back {
        let font_scale = state.settings.lock().map(|s| s.font_scale).unwrap_or(1.0);
        apply_settings(&app, &state, ProjectionSettings { font_scale, theme: themes::default_theme() })?;
    }
    Ok(())
}

// ---- Planning Center Online import ----

/// Fetch a Planning Center plan's item list (songs, headers, …). Blocking HTTP
/// on a worker thread. Requires the operator's own PCO Personal Access Token.
#[tauri::command]
pub async fn pco_import_plan(
    app_id: String,
    secret: String,
    service_type_id: String,
    plan_id: String,
) -> Result<Vec<crate::planning_center::PlanItem>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::planning_center::fetch_plan(&app_id, &secret, &service_type_id, &plan_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- PJLink network projector control ----

/// Send one PJLink command (e.g. "%1POWR 1" on, "%1POWR 0" off, "%1AVMT 31"
/// blank, "%1AVMT 30" unblank, "%1POWR ?" query) to a projector on the LAN and
/// return its response. Runs on a blocking thread so a slow/absent projector
/// can't freeze the UI.
#[tauri::command]
pub async fn pjlink_command(
    host: String,
    port: u16,
    password: String,
    body: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::pjlink::send(&host, port, &password, &body))
        .await
        .map_err(|e| e.to_string())?
}

// ---- Lower-third alerts (overlay on the congregation screen) ----

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn emit_alert(app: &tauri::AppHandle, alert: &Alert) -> Result<(), String> {
    app.emit_to("projection", "set-alert", alert.clone())
        .map_err(|e| e.to_string())
}

/// The current alert, so the projection window can show it on (re)load.
#[tauri::command]
pub fn get_alert(state: tauri::State<'_, AppState>) -> Alert {
    state.alert.lock().map(|a| a.clone()).unwrap_or_default()
}

/// Overlay an alert band over whatever is live. `seconds > 0` auto-dismisses;
/// 0 keeps it up until cleared. Empty text clears it.
#[tauri::command]
pub fn show_alert(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    text: String,
    seconds: i64,
) -> Result<(), String> {
    let text = text.trim().to_string();
    let until_ms = if text.is_empty() || seconds <= 0 { 0 } else { now_ms() + seconds * 1000 };
    let alert = Alert { text, until_ms };
    if let Ok(mut a) = state.alert.lock() {
        *a = alert.clone();
    }
    emit_alert(&app, &alert)
}

/// Clear any live alert.
#[tauri::command]
pub fn clear_alert(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let alert = Alert::default();
    if let Ok(mut a) = state.alert.lock() {
        *a = alert.clone();
    }
    emit_alert(&app, &alert)
}

// ---- Audio ----
//
// Sound runs beside the projection rather than through it. Every command here
// leaves `current` alone, which is the whole point: an offering track plays
// while the giving verse stays on the wall.

fn emit_audio(app: &tauri::AppHandle, audio: &crate::events::AudioState) -> Result<(), String> {
    // The projection window owns the element, because it is the window that
    // lives for the whole service. The operator's console can be reloaded, and
    // the music must not stop when it is.
    app.emit_to("projection", "set-audio", audio.clone()).map_err(|e| e.to_string())
}

/// What is playing, so the projection window can pick it up again on (re)load
/// and the console can draw the transport it is actually in.
#[tauri::command]
pub fn get_audio(state: tauri::State<'_, AppState>) -> crate::events::AudioState {
    state.audio.lock().map(|a| a.clone()).unwrap_or_default()
}

fn set_audio(app: &tauri::AppHandle, next: crate::events::AudioState) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        // Semicolon matters: as the block's tail expression the lock guard would
        // outlive `state` and fail to borrow-check.
        if let Ok(mut a) = state.audio.lock() {
            *a = next.clone();
        };
    }
    emit_audio(app, &next)
}

/// Start a library item playing, keeping the volume the operator last set. Used
/// by the console, the run order and the LAN remote alike.
pub fn play_audio_handle(app: &tauri::AppHandle, path: &str, title: &str) -> Result<(), String> {
    let volume = app
        .state::<AppState>()
        .audio
        .lock()
        .map(|a| a.volume)
        .unwrap_or(1.0);
    set_audio(
        app,
        crate::events::AudioState {
            src: path.to_string(),
            title: title.to_string(),
            paused: false,
            looping: false,
            volume,
        },
    )
}

/// Play one item from the media library by id. Errors if it is not a sound file
/// — putting a picture through the speakers is a mistake worth naming.
#[tauri::command]
pub fn play_audio(app: tauri::AppHandle, id: i64) -> Result<crate::media::MediaItem, String> {
    let item = crate::media::list(&app)
        .into_iter()
        .find(|m| m.id == id)
        .ok_or_else(|| "That item is no longer in the library.".to_string())?;
    if item.kind != "audio" {
        return Err(format!("'{}' is not a sound file.", item.title));
    }
    if !item.present {
        return Err(format!("'{}' is no longer at {}", item.title, item.path));
    }
    play_audio_handle(&app, &item.path, &item.title)?;
    Ok(item)
}

/// Pause/resume, loop, and volume, in one call so the transport can never end
/// up half-applied. Volume is clamped: a value outside 0..1 is a bug upstream,
/// not an instruction to deafen a congregation.
#[tauri::command]
pub fn set_audio_playback(
    app: tauri::AppHandle,
    paused: bool,
    looping: bool,
    volume: f32,
) -> Result<(), String> {
    let current = get_audio(app.state::<AppState>());
    if current.src.is_empty() {
        return Err("No sound is loaded.".into());
    }
    set_audio(
        &app,
        crate::events::AudioState {
            paused,
            looping,
            volume: volume.clamp(0.0, 1.0),
            ..current
        },
    )
}

/// Jump the playing track to a position, as an instant rather than a condition
/// (the same reasoning as `seek_video`).
#[tauri::command]
pub fn seek_audio(app: tauri::AppHandle, position_ms: i64) -> Result<(), String> {
    app.emit_to("projection", "audio-seek", position_ms.max(0)).map_err(|e| e.to_string())
}

/// Stop and unload, keeping the operator's volume for whatever plays next.
#[tauri::command]
pub fn stop_audio(app: tauri::AppHandle) -> Result<(), String> {
    let volume = get_audio(app.state::<AppState>()).volume;
    set_audio(&app, crate::events::AudioState { volume, ..Default::default() })
}

// ---- Stage / confidence monitor ----

/// Emit the current stage state to the stage window (also fine while it's hidden).
fn emit_stage(app: &tauri::AppHandle, info: &StageInfo) -> Result<(), String> {
    app.emit_to("stage", "set-stage", info.clone())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_stage(state: tauri::State<'_, AppState>) -> StageInfo {
    state.stage.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Set the current + next lines shown on the stage monitor. Preserves any
/// active operator message.
#[tauri::command]
pub fn set_stage(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    current: Option<StageSlot>,
    next: Option<StageSlot>,
) -> Result<(), String> {
    let info = {
        let mut s = state.stage.lock().map_err(|e| e.to_string())?;
        s.current = current;
        s.next = next;
        s.clone()
    };
    emit_stage(&app, &info)
}

/// Set the stage monitor's two slots from a handle, for the surfaces that run
/// off the console's main thread (the media slideshow, the LAN remote).
pub(crate) fn set_stage_handle(
    app: &tauri::AppHandle,
    current: Option<StageSlot>,
    next: Option<StageSlot>,
) {
    let state = app.state::<AppState>();
    let info = {
        let Ok(mut s) = state.stage.lock() else { return };
        s.current = current;
        s.next = next;
        s.clone()
    };
    let _ = emit_stage(app, &info);
}

/// Push (or clear, with an empty string) a private message to the platform team.
#[tauri::command]
pub fn set_stage_message(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    message: String,
) -> Result<(), String> {
    let info = {
        let mut s = state.stage.lock().map_err(|e| e.to_string())?;
        s.message = message;
        s.clone()
    };
    emit_stage(&app, &info)
}

/// Start/stop the stage timer. `mode` is "countup" (elapsed from now),
/// "countdown" (`seconds` from now), or "off". Shows only on the stage monitor.
#[tauri::command]
pub fn set_stage_timer(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mode: String,
    seconds: i64,
) -> Result<(), String> {
    let anchor_ms = match mode.as_str() {
        "countup" => now_ms(),
        "countdown" => now_ms() + seconds.max(0) * 1000,
        _ => 0,
    };
    let info = {
        let mut s = state.stage.lock().map_err(|e| e.to_string())?;
        s.timer = crate::events::StageTimer { mode, anchor_ms };
        s.clone()
    };
    emit_stage(&app, &info)
}

/// Peek at the next verse from the current cursor without projecting it or
/// moving the cursor — used to populate the stage "next" preview.
#[tauri::command]
pub fn peek_next(state: tauri::State<'_, AppState>) -> Option<VersePayload> {
    let cur = state.cursor.lock().ok().and_then(|c| c.clone())?;
    let tr = state.active_translation();
    let db = state.db.lock().ok()?;
    let (osis, ch, v) = compute_nav(&db, &tr, &cur, "next-verse").ok().flatten()?;
    let rec = db.verse_at(&tr, &osis, ch, v).ok().flatten()?;
    Some(build_payload(rec))
}

// ---- Live listening (STT) ----

fn first_existing(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.exists())
}

/// Locate the whisper model + binary. Searched in order: the packaged app's
/// resource dir (`<res>/models`, `<res>/bin` — how shipped installers carry
/// them), then the dev project's `models/` and `bin/` dirs. `kind` selects the
/// flavor's model: "base" (normal), "small" (best), or "tiny" (low-end PCs).
fn resolve_model_and_binary(
    res_dir: Option<&Path>,
    kind: &str,
) -> Result<(PathBuf, PathBuf), String> {
    let dev_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf);

    let mut model_dirs: Vec<PathBuf> = Vec::new();
    if let Some(r) = res_dir {
        model_dirs.push(r.join("models"));
    }
    if let Some(root) = &dev_root {
        model_dirs.push(root.join("models"));
    }

    let named = format!("ggml-{kind}.en.bin");
    let fallbacks = ["ggml-base.en.bin", "ggml-small.en.bin", "ggml-tiny.en.bin", "ggml-medium.en.bin"];
    let model = model_dirs
        .iter()
        .find_map(|d| {
            let exact = d.join(&named);
            if exact.exists() { Some(exact) } else { first_existing(d, &fallbacks) }
        })
        .ok_or("No whisper model found (looked in bundled resources and the project 'models' folder).")?;

    let binary = bin_roots(res_dir)
        .iter()
        .find_map(|root| {
            // The chosen backend first (CPU until something faster is installed and
            // measured), then any other backend that is present, then the flat
            // layout every installer shipped so far.
            let want = crate::accel::chosen().unwrap_or(crate::accel::Backend::Cpu);
            let dirs = std::iter::once(want)
                .chain(crate::accel::Backend::RANKED)
                .filter_map(|b| crate::accel::dir_for(root, b));
            for d in dirs {
                if let Some(exe) = first_existing(&d, &["whisper-cli.exe", "main.exe", "whisper.exe"]) {
                    return Some(exe);
                }
            }
            first_existing(root, &["whisper-cli.exe", "main.exe", "whisper.exe"])
        })
        .unwrap_or_else(|| PathBuf::from("whisper-cli")); // else rely on PATH

    Ok((model, binary))
}

/// Where whisper builds are kept: the packaged app's resources, then the dev
/// project's `bin/`.
pub(crate) fn bin_roots(res_dir: Option<&Path>) -> Vec<PathBuf> {
    let dev_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf);
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(r) = res_dir {
        roots.push(r.join("bin"));
    }
    if let Some(root) = dev_root {
        roots.push(root.join("bin"));
    }
    roots
}

// ---- Where whisper runs ----------------------------------------------------

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccelOption {
    pub key: String,
    pub label: String,
    /// This build ships a whisper build for it.
    pub installed: bool,
    /// This machine's drivers can run it. A build can ship CUDA to a laptop with
    /// an AMD card in it, so the two are separate answers.
    pub usable: bool,
    /// Milliseconds per utterance, if it has been measured here.
    pub measured_ms: Option<u64>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccelStatus {
    /// "auto", or a forced backend key.
    pub preference: String,
    /// What that resolves to right now.
    pub chosen: String,
    pub chosen_label: String,
    pub threads: usize,
    pub options: Vec<AccelOption>,
}

fn accel_status_from(app: &tauri::AppHandle) -> Result<AccelStatus, String> {
    let state = app.state::<AppState>();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let res_dir = app.path().resource_dir().ok();
    let roots = bin_roots(res_dir.as_deref());
    let root = roots.first().cloned().unwrap_or_default();
    // The dev tree and a packaged install both count: whichever actually holds a
    // whisper build is the one being asked about.
    let root = roots
        .iter()
        .find(|r| !crate::accel::available(r).is_empty())
        .cloned()
        .unwrap_or(root);

    let chosen = crate::accel::refresh(&db, &root);
    let preference = db
        .get_setting(crate::accel::SETTING_PREFERENCE)
        .unwrap_or_else(|| "auto".into());
    let timings: std::collections::HashMap<String, u64> = db
        .get_setting("accel_measured_ms")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let options = crate::accel::Backend::RANKED
        .iter()
        .map(|&b| AccelOption {
            key: b.key().into(),
            label: b.label().into(),
            installed: crate::accel::dir_for(&root, b).is_some(),
            usable: crate::accel::available(&root).contains(&b),
            measured_ms: timings.get(b.key()).copied(),
        })
        .collect();

    Ok(AccelStatus {
        preference,
        chosen: chosen.key().into(),
        chosen_label: chosen.label().into(),
        threads: crate::stt::threads().parse().unwrap_or(4),
        options,
    })
}

/// What this machine can run whisper on, and what it is running it on.
#[tauri::command]
pub fn accel_status(app: tauri::AppHandle) -> Result<AccelStatus, String> {
    accel_status_from(&app)
}

/// "auto", or a backend key to force. Forcing exists because a graphics driver
/// that misbehaves under load is a real thing, and when it happens mid-service the
/// operator needs a way back to the processor that is not a reinstall.
#[tauri::command]
pub fn set_accel_preference(app: tauri::AppHandle, preference: String) -> Result<AccelStatus, String> {
    let parsed = crate::accel::Preference::parse(&preference);
    {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.set_setting(crate::accel::SETTING_PREFERENCE, &parsed.key())
            .map_err(|e| e.to_string())?;
    }
    accel_status_from(&app)
}

/// Time every backend this machine can run and keep the fastest.
///
/// Runs on its own thread and reports each trial as it finishes, because it takes
/// the better part of a minute and a frozen window looks like a crash. Refuses
/// while listening: half a dozen trial transcriptions would compete with the very
/// service they are meant to speed up.
#[tauri::command]
pub fn measure_accel(app: tauri::AppHandle, model: Option<String>) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state.listening.load(Ordering::SeqCst) {
        return Err("Stop listening first — measuring runs several test transcriptions, which would slow down the service it is trying to speed up.".into());
    }
    let kind = model.unwrap_or_else(|| crate::flavor::default_model().to_string());
    let res_dir = app.path().resource_dir().ok();
    let (model_path, _) = resolve_model_and_binary(res_dir.as_deref(), &kind)?;
    let roots = bin_roots(res_dir.as_deref());
    let root = roots
        .iter()
        .find(|r| !crate::accel::available(r).is_empty())
        .cloned()
        .ok_or("No whisper build was found to measure.")?;
    let captures = crate::capture::dir(&app);

    std::thread::spawn(move || {
        let report = |t: &crate::accel_probe::Trial| {
            let _ = app.emit(
                "accel-trial",
                serde_json::json!({
                    "backend": t.backend.key(),
                    "label": t.backend.label(),
                    "threads": t.threads,
                    "ms": t.ms,
                }),
            );
        };
        match crate::accel_probe::measure(&root, &model_path, captures.as_deref(), report) {
            Ok(measured) => {
                let best = match measured.best() {
                    Some(b) => b,
                    None => return,
                };
                let per_backend = measured.by_backend();
                let state = app.state::<AppState>();
                if let Ok(db) = state.db.lock() {
                    let _ = db.set_setting(crate::accel::SETTING_MEASURED, best.backend.key());
                    let _ = db.set_setting(crate::accel::SETTING_THREADS, &best.threads.to_string());
                    if let Ok(json) = serde_json::to_string(&per_backend) {
                        let _ = db.set_setting("accel_measured_ms", &json);
                    }
                    crate::accel::refresh(&db, &root);
                }
                let _ = app.emit(
                    "accel-measured",
                    serde_json::json!({
                        "backend": best.backend.key(),
                        "label": best.backend.label(),
                        "threads": best.threads,
                        "ms": best.ms,
                        // False means no real utterance had been captured yet, so the
                        // ranking is a rough guide rather than a figure to quote.
                        "realAudio": measured.real_audio,
                    }),
                );
            }
            Err(e) => {
                let _ = app.emit("accel-error", e);
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub fn start_listening(app: tauri::AppHandle, model: Option<String>) -> Result<(), String> {
    begin_listening(&app, model.as_deref())
}

/// Start the listen loop for `kind` (default "base"). Shared by the console button
/// and the phone remote — the operator is usually standing at the projector when the
/// preacher steps up, not sitting at the laptop.
pub(crate) fn begin_listening(app: &tauri::AppHandle, model: Option<&str>) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state.listening.load(Ordering::SeqCst) {
        return Ok(());
    }
    let kind = model.unwrap_or("base").to_string();
    let res_dir = app.path().resource_dir().ok();
    let (model, binary) = resolve_model_and_binary(res_dir.as_deref(), &kind)?;
    // Use whatever calibration found for the speaker who is preaching today.
    let decode = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let who = crate::calibrate::active_profile(&db);
        crate::calibrate::load(&db, &model, &who)
    };
    let input = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        chosen_input(&db)
    };
    // No input, no listening. Quietly opening the laptop's own microphone would hear
    // the room instead of the preacher and look for all the world like it was working.
    // Opening it because the operator asked for it, to demonstrate the app, is a
    // different thing — and that is what `Input::room_mic_ok` carries.
    if input.name.is_none() {
        return Err(
            "Choose the sound input first — the feed from the sound desk, under Live \
             listening → Sound input."
                .into(),
        );
    }
    // Everything learned about this speaker, applied before a word is heard: the book
    // names they get misheard as, how loud their room is, and the version they read
    // from.
    let room = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let who = crate::calibrate::active_profile(&db);
        crate::books::set_learned_names(crate::learn::book_names(&db, &who));
        if let Some(code) = crate::learn::load_translation(&db, &who) {
            if db.list_translations().map(|t| t.iter().any(|(c, _)| *c == code)).unwrap_or(false) {
                if let Ok(mut tr) = state.translation.lock() {
                    *tr = code.clone();
                }
                let _ = app.emit("translation-changed", &code);
            }
        }
        crate::learn::load_room(&db, &who)
    };
    // If recording is on, capture this whole service to the active speaker's rolling
    // folder so it can be learned from later. On-device only.
    let record = if state.recording.load(Ordering::SeqCst) {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let who = crate::calibrate::active_profile(&db);
        // The last word on whether a preacher is recorded, checked against the speaker
        // who is actually about to preach — not whoever was selected when the switch
        // was flipped.
        if !crate::sessions::consented(&db, &who) {
            drop(db);
            state.recording.store(false, Ordering::SeqCst);
            return Err(format!(
                "{who} has not been opted in to being recorded, so this service will not be \
                 recorded. Start listening again to go ahead without it."
            ));
        }
        if let Ok(mut m) = state.moments.lock() {
            m.clear(); // fresh log for this service
        }
        let keep = crate::sessions::window_size(&db);
        app.path().app_data_dir().ok().map(|base| crate::sessions::RecordTarget {
            dir: crate::sessions::dir_for(&base, &who),
            name: crate::sessions::now_stamp(),
            keep,
        })
    } else {
        None
    };
    // Claim the listen loop atomically. The early return above is only a courtesy: two
    // starts arriving together (a double-click, or the console and the phone remote at
    // once) could both pass it and spawn a loop each — two mics open, doubled events,
    // and two recorders writing one service.
    if state
        .listening
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(()); // already listening — the same answer the early return gives
    }
    let flag = state.listening.clone();
    let app2 = app.clone();
    std::thread::spawn(move || {
        crate::audio::run_listen_loop(app2, flag, model, binary, decode, input, room, record)
    });
    Ok(())
}

/// Turn recording of services on or off (for learning). Off by default; stays on until
/// switched off. Nothing is uploaded — recordings live only on this machine.
///
/// Recording is refused for a speaker who has not been opted in. The console does not
/// offer the switch in that case, so this is the guard behind the guard: whatever
/// route arrives here, an un-consented preacher is not recorded.
#[tauri::command]
pub fn set_recording(on: bool, state: tauri::State<'_, AppState>) -> Result<(), String> {
    if on {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let who = crate::calibrate::active_profile(&db);
        if !crate::sessions::consented(&db, &who) {
            return Err(format!(
                "Turn on “Record services to improve {who}” first — recording a preacher is \
                 their call to make."
            ));
        }
    }
    state.recording.store(on, Ordering::SeqCst);
    Ok(())
}

/// Whether the speaker preaching today has been opted in to being recorded.
#[tauri::command]
pub fn recording_consent(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let who = crate::calibrate::active_profile(&db);
    Ok(crate::sessions::consented(&db, &who))
}

/// Opt today's speaker in or out of having their services recorded. Opting out also
/// switches off any recording armed for them — the answer takes effect at once.
#[tauri::command]
pub fn set_recording_consent(on: bool, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let who = crate::calibrate::active_profile(&db);
    crate::sessions::set_consent(&db, &who, on).map_err(|e| e.to_string())?;
    if !on {
        state.recording.store(false, Ordering::SeqCst);
    }
    Ok(())
}

/// Delete every recording of today's speaker and forget they were opted in. Returns
/// how many services were thrown away.
#[tauri::command]
pub fn forget_recordings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    let dir = session_dir(&app, &state)?;
    state.recording.store(false, Ordering::SeqCst);
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let who = crate::calibrate::active_profile(&db);
    let gone = crate::sessions::forget(&db, &dir, &who).map_err(|e| e.to_string())?;
    // A proposal drawn from recordings that no longer exist has no evidence behind it.
    crate::relearn::drop_proposal(&db, &who).map_err(|e| e.to_string())?;
    Ok(gone)
}

#[tauri::command]
pub fn recording_enabled(state: tauri::State<'_, AppState>) -> bool {
    state.recording.load(Ordering::SeqCst)
}

/// The folder of recorded services for the speaker preaching today.
fn session_dir(app: &tauri::AppHandle, state: &AppState) -> Result<std::path::PathBuf, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let who = crate::calibrate::active_profile(&db);
    Ok(crate::sessions::dir_for(&base, &who))
}

/// The recorded services for today's speaker, newest first, with what happened in each.
/// This is what the end-of-service review reads.
#[tauri::command]
pub fn review_sessions(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::sessions::SessionSummary>, String> {
    Ok(crate::sessions::summaries(&session_dir(&app, &state)?))
}

/// Approve a reviewed service: the operator has seen what it captured and is content
/// for the app to learn from it. Until this happens the service sits on disk and
/// teaches the app nothing.
///
/// Approval is of the whole service, because learning listens to the whole recording —
/// it works out for itself what was read aloud rather than being handed the moments.
/// The moments are the operator's evidence for deciding, and the record of what the app
/// did; they are left exactly as they were captured.
#[tauri::command]
pub fn approve_session(
    name: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let dir = session_dir(&app, &state)?;
    let audio = crate::sessions::audio_named(&dir, &name)
        .ok_or_else(|| format!("That recording is no longer here ({name})."))?;
    let mut labels = crate::sessions::read_labels(&audio);
    labels.approved = true;
    crate::sessions::write_labels(&audio, &labels).map_err(|e| e.to_string())
}

/// Throw a recorded service away — the operator's answer to "don't keep this one".
#[tauri::command]
pub fn discard_session(
    name: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let dir = session_dir(&app, &state)?;
    let audio = crate::sessions::audio_named(&dir, &name)
        .ok_or_else(|| format!("That recording is no longer here ({name})."))?;
    crate::sessions::discard(&audio);
    Ok(())
}

// ---- learning from the church's own approved services -----------------------------

/// The services this speaker has recorded and approved, newest first, as paths.
fn approved_audio(dir: &Path) -> Vec<(String, std::path::PathBuf)> {
    crate::sessions::list_audio(dir)
        .into_iter()
        .filter(|a| crate::sessions::read_labels(a).approved)
        .map(|a| {
            let name = a.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            (name, a)
        })
        .collect()
}

/// What the console needs to show about learning: whether it could run now, what it
/// is waiting for if not, and anything it has to say for itself.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningStatus {
    pub profile: String,
    /// Approved services available for this speaker.
    pub approved: usize,
    pub running: bool,
    /// Why learning would not start right now; null when it could.
    pub blocked: Option<String>,
    pub proposal: Option<crate::relearn::Proposal>,
    /// Is there a version to go back to, and a shipped one to fall back on?
    pub can_rollback: bool,
    pub can_reset: bool,
}

fn last_learn_key(profile: &str) -> String {
    format!("last_learn:{profile}")
}

/// Milliseconds since this speaker was last learned from.
fn since_last_learn(db: &Db, profile: &str) -> Option<u64> {
    let then: u64 = db.get_setting(&last_learn_key(profile))?.parse().ok()?;
    let now: u64 = crate::sessions::now_stamp().parse().ok()?;
    Some(now.saturating_sub(then))
}

/// Everything the console needs to decide what to offer the operator.
#[tauri::command]
pub fn learning_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<LearningStatus, String> {
    let dir = session_dir(&app, &state)?;
    let approved = approved_audio(&dir).len();
    let (profile, proposal, since, can_rollback) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let profile = crate::calibrate::active_profile(&db);
        (
            profile.clone(),
            crate::relearn::load_proposal(&db, &profile),
            since_last_learn(&db, &profile),
            crate::relearn::has_previous(&db, &profile),
        )
    };
    let projecting = state
        .current
        .lock()
        .map(|c| crate::idle::is_projecting(&c))
        .unwrap_or(false);
    let now = crate::idle::Now {
        listening: state.listening.load(Ordering::SeqCst),
        projecting,
        on_mains: crate::idle::on_mains(),
        learning: state.learning.load(Ordering::SeqCst),
        approved,
        since_last_ms: since,
    };
    Ok(LearningStatus {
        can_reset: crate::relearn::has_baked(&profile),
        profile,
        approved,
        running: now.learning,
        blocked: crate::idle::blocked(&now).map(|b| b.message().to_string()),
        proposal,
        can_rollback,
    })
}

/// Learn from this speaker's approved services, in the background, and leave a proposal
/// for the operator. Nothing about the app changes until they accept it.
#[tauri::command]
pub fn learn_now(app: tauri::AppHandle) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let dir = session_dir(&app, &state)?;
        let approved = approved_audio(&dir).len();
        let projecting =
            state.current.lock().map(|c| crate::idle::is_projecting(&c)).unwrap_or(false);
        let since = {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            let who = crate::calibrate::active_profile(&db);
            since_last_learn(&db, &who)
        };
        // The operator asking for it now is reason enough to ignore the cooldown; the
        // rest of the gate still stands, because it is about the machine, not the timing.
        let now = crate::idle::Now {
            listening: state.listening.load(Ordering::SeqCst),
            projecting,
            on_mains: crate::idle::on_mains(),
            learning: state.learning.load(Ordering::SeqCst),
            approved,
            since_last_ms: since,
        };
        if let Some(b) = crate::idle::blocked(&now) {
            if b != crate::idle::Blocked::TooSoon {
                return Err(b.message().to_string());
            }
        }
        // Claim the pass atomically. Two clicks arriving together must not both get
        // past the check above and start their own hours-long decode of the same
        // services — and the first to finish would clear the flag under the second,
        // stopping it with a reason that was never true.
        if state
            .learning
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(crate::idle::Blocked::AlreadyLearning.message().to_string());
        }
    }
    let app2 = app.clone();
    std::thread::spawn(move || {
        let outcome = learn_from_approved(&app2);
        app2.state::<AppState>().learning.store(false, Ordering::SeqCst);
        match outcome {
            Ok(Some(p)) => {
                let _ = app2.emit("learn-proposal", p);
            }
            Ok(None) => {
                let _ = app2.emit("learn-idle-done", "Nothing new to learn from those services.");
            }
            Err(e) if e == crate::learn::CANCELLED => {
                let _ = app2.emit("learn-idle-done", "Stopped — the machine was needed.");
            }
            Err(e) => {
                let _ = app2.emit("learn-error", e);
            }
        }
    });
    Ok(())
}

/// Stop a background pass. Also what the app does to itself the moment the operator
/// starts a service: nothing is worth interrupting that.
#[tauri::command]
pub fn stop_learning(state: tauri::State<'_, AppState>) {
    state.learning.store(false, Ordering::SeqCst);
}

/// The pass itself: learn from the approved services and work out what, if anything,
/// is worth proposing. Runs on a background thread and holds no lock while decoding.
fn learn_from_approved(app: &tauri::AppHandle) -> Result<Option<crate::relearn::Proposal>, String> {
    use crate::learn::Progress;
    let state = app.state::<AppState>();
    let dir = session_dir(app, &state)?;
    let approved = approved_audio(&dir);
    if approved.is_empty() {
        return Ok(None);
    }
    let names: Vec<String> = approved.iter().map(|(n, _)| n.clone()).collect();
    let paths: Vec<String> = approved.iter().map(|(_, p)| p.to_string_lossy().into_owned()).collect();

    let profile = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        crate::calibrate::active_profile(&db)
    };
    let res_dir = app.path().resource_dir().ok();
    let kind = crate::flavor::default_model().to_string();
    let (target_model, binary) = resolve_model_and_binary(res_dir.as_deref(), &kind)?;
    let (scout_model, _) = resolve_model_and_binary(res_dir.as_deref(), "base")
        .unwrap_or_else(|_| (target_model.clone(), binary.clone()));
    let incumbent = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        Some(crate::calibrate::load(&db, &target_model, &profile))
    };

    // Give the machine back the instant the church wants it — a service starting, or
    // anything going up on the screen, outranks learning without discussion.
    let keep_going = {
        let app = app.clone();
        move || {
            let s = app.state::<AppState>();
            s.learning.load(Ordering::SeqCst)
                && !s.listening.load(Ordering::SeqCst)
                && !s.current.lock().map(|c| crate::idle::is_projecting(&c)).unwrap_or(false)
        }
    };
    let say = {
        let app = app.clone();
        move |stage: &str, done: usize, total: usize, base: f32, share: f32| {
            let frac = if total == 0 { 0.0 } else { done as f32 / total as f32 };
            let overall = (base + share * frac).clamp(0.0, 1.0);
            let _ = app.emit(
                "learn-idle-progress",
                Progress {
                    stage: stage.to_string(),
                    done,
                    total,
                    percent: (overall * 100.0) as u8,
                    seconds_left: 0,
                },
            );
        }
    };
    let reading = |text: &str| state.db.lock().ok().and_then(|db| crate::learn::reading_of(&db, text));
    let learned =
        crate::learn::run(&scout_model, &target_model, &binary, &paths, incumbent, keep_going, reading, say)?;

    let db = state.db.lock().map_err(|e| e.to_string())?;
    // Mark the attempt however it turns out: a pass that found nothing worth proposing
    // should not be repeated on the same services tomorrow morning.
    db.set_setting(&last_learn_key(&profile), &crate::sessions::now_stamp())
        .map_err(|e| e.to_string())?;
    let model_file = target_model
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| crate::flavor::model_file(&kind));
    let Some(p) = crate::relearn::propose(&db, &profile, &model_file, &learned, names) else {
        crate::relearn::drop_proposal(&db, &profile).map_err(|e| e.to_string())?;
        return Ok(None);
    };
    crate::relearn::save_proposal(&db, &p).map_err(|e| e.to_string())?;
    Ok(Some(p))
}

/// Take up what the last pass proposed. What it replaces is kept, so this is safe to
/// try: `rollback_profile` puts it straight back.
#[tauri::command]
pub fn accept_proposal(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let profile = crate::calibrate::active_profile(&db);
    let took = crate::relearn::accept(&db, &profile).map_err(|e| e.to_string())?;
    if took {
        // In force straight away, without waiting for the next listen.
        crate::books::set_learned_names(crate::learn::book_names(&db, &profile));
        let _ = app.emit("profile-changed", &profile);
    }
    Ok(took)
}

/// Turn down what the last pass proposed. The services stay; only the suggestion goes.
#[tauri::command]
pub fn reject_proposal(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let profile = crate::calibrate::active_profile(&db);
    crate::relearn::drop_proposal(&db, &profile).map_err(|e| e.to_string())
}

/// Put back the tuning in force before the last change.
#[tauri::command]
pub fn rollback_profile(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let profile = crate::calibrate::active_profile(&db);
    let done = crate::relearn::rollback(&db, &profile).map_err(|e| e.to_string())?;
    if done {
        crate::books::set_learned_names(crate::learn::book_names(&db, &profile));
        let _ = app.emit("profile-changed", &profile);
    }
    Ok(done)
}

/// Put this speaker back exactly as the app shipped them, discarding everything this
/// machine has learned. The floor under all of it.
#[tauri::command]
pub fn reset_profile_to_baked(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let profile = crate::calibrate::active_profile(&db);
    let done = crate::relearn::reset_to_baked(&db, &profile).map_err(|e| e.to_string())?;
    if done {
        crate::relearn::drop_proposal(&db, &profile).map_err(|e| e.to_string())?;
        crate::books::set_learned_names(crate::learn::book_names(&db, &profile));
        let _ = app.emit("profile-changed", &profile);
    }
    Ok(done)
}

/// Log one moment from the live service (auto-projected, operator-corrected, confirmed).
/// Kept in memory and written into the session recording when listening stops.
///
/// Ignored unless a service is actually being recorded. The console already gates this,
/// but a moment ends up in a person's session file, so the rule that nothing is written
/// about an unrecorded service is enforced here rather than trusted to the caller.
#[tauri::command]
pub fn record_moment(moment: crate::sessions::Moment, state: tauri::State<'_, AppState>) {
    if !state.recording.load(Ordering::SeqCst) || !state.listening.load(Ordering::SeqCst) {
        return;
    }
    if let Ok(mut m) = state.moments.lock() {
        m.push(moment);
    }
}

#[tauri::command]
pub fn stop_listening(state: tauri::State<'_, AppState>) {
    state.listening.store(false, Ordering::SeqCst);
}

// ---- Voice calibration ------------------------------------------------------
// Tune the recognizer to this speaker's voice, mic and room. The clips, the sweep
// and the result all stay on the machine.

// ---- Audio input ------------------------------------------------------------
// The laptop microphone hears the room: reverb, the congregation, whatever the PA
// gives back. A feed from the sound desk carries the preacher's own microphone,
// already mixed — no room in it at all. Everything this app has been measured
// against is that kind of signal, so being able to choose the input is the single
// biggest thing the operator can do for accuracy.

#[tauri::command]
pub fn audio_inputs(state: tauri::State<'_, AppState>) -> Result<AudioInputs, String> {
    let input = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        chosen_input(&db)
    };
    let (all, room_mics) = crate::audio::input_devices();
    // Only true when the app really would open the room mic — same test the audio
    // path applies, so the warning cannot disagree with what is happening.
    let on_room_mic = input
        .name
        .as_deref()
        .map(|n| input.room_mic_ok && crate::audio::is_machine_microphone(n))
        .unwrap_or(false);
    Ok(AudioInputs { chosen: input.name, all, room_mics, on_room_mic })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioInputs {
    /// None = nothing chosen yet. There is no default: see `audio::resolve_input`.
    pub chosen: Option<String>,
    /// Feeds from the sound desk — what the app is for.
    pub all: Vec<String>,
    /// This machine's own microphones, listed apart from the desk feeds and never
    /// mixed in with them. Choosing one takes a deliberate second step.
    pub room_mics: Vec<String>,
    /// Is the app currently pointed at the room rather than the desk? Drives the
    /// warning the operator can see without going looking for it.
    pub on_room_mic: bool,
}

/// The chosen input, read as one thing: the device name and whether the room mic was
/// permitted. Everything that opens audio goes through here.
///
/// The permission lives in its own setting rather than being inferred from the device
/// name, so that a name arriving by any other route — an old database, a hand edit, a
/// device that changed its name — cannot grant itself permission to listen to the room.
fn chosen_input(db: &crate::db::Db) -> crate::audio::Input {
    crate::audio::Input {
        name: db.get_setting("input_device").filter(|s| !s.is_empty()),
        room_mic_ok: db.get_setting("input_room_mic_ok").as_deref() == Some("1"),
    }
}

/// Choose the input. `room_mic` is the operator saying, in as many words, "yes, the
/// laptop's own microphone, I know it hears the room" — it is only honoured for a
/// device that really is one, and it is cleared the moment anything else is picked.
#[tauri::command]
pub fn set_audio_input(
    name: Option<String>,
    room_mic: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let name = name.filter(|n| !n.is_empty());
    let ok = room_mic.unwrap_or(false)
        && name.as_deref().map(crate::audio::is_machine_microphone).unwrap_or(false);
    db.set_setting("input_room_mic_ok", if ok { "1" } else { "" }).map_err(|e| e.to_string())?;
    db.set_setting("input_device", name.as_deref().unwrap_or("")).map_err(|e| e.to_string())
}

/// Listen for a couple of seconds and report the loudest level heard (0..1), so the
/// operator can confirm sound is actually arriving from the desk before the service
/// rather than discovering it mid-sermon.
#[tauri::command]
pub async fn test_audio_input(app: tauri::AppHandle) -> Result<f32, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::audio::input_level(&selected_input(&app)?, 3.0)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn selected_input(app: &tauri::AppHandle) -> Result<crate::audio::Input, String> {
    let state = app.state::<AppState>();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    Ok(chosen_input(&db))
}

// ---- Learning a preacher from their recordings ------------------------------

/// Teach the app a preacher from recordings of them preaching.
///
/// Twelve read lines at a laptop tune the app for someone reading a script. Sermons
/// tune it for a preacher: their pace, their accent, the way they name references,
/// and the signal path the church actually uses. Several recordings beat one — each
/// sermon carries only a handful of passages read aloud, and settings chosen on six
/// data points are settings chosen on one Sunday.
///
/// Ground truth costs nothing: whatever they read out is a verse a human operator got
/// right that day, so nobody has to mark anything up.
///
/// Everything runs on this machine, and everything is written to the selected
/// speaker's profile alone.
#[tauri::command]
pub async fn learn_from_recordings(
    app: tauri::AppHandle,
    paths: Vec<String>,
    model: Option<String>,
) -> Result<crate::learn::LearnResult, String> {
    tauri::async_runtime::spawn_blocking(move || learn_sermons(app, paths, model))
        .await
        .map_err(|e| e.to_string())?
}

/// Weight of the discovery pass in the overall progress bar. Listening to every
/// utterance is the bulk of the work; the sweep only revisits the clips that matter.
fn learn_sermons(
    app: tauri::AppHandle,
    paths: Vec<String>,
    model: Option<String>,
) -> Result<crate::learn::LearnResult, String> {
    use crate::learn::{self, Progress};
    use std::time::Instant;

    if paths.is_empty() {
        return Err("Pick at least one recording.".into());
    }

    let started = Instant::now();
    let say = {
        let app = app.clone();
        move |stage: &str, done: usize, total: usize, base: f32, share: f32| {
            let frac = if total == 0 { 0.0 } else { done as f32 / total as f32 };
            let overall = (base + share * frac).clamp(0.0, 1.0);
            let elapsed = started.elapsed().as_secs_f32();
            let left = if overall > 0.02 { elapsed / overall - elapsed } else { 0.0 };
            let _ = app.emit(
                "learn-progress",
                Progress {
                    stage: stage.to_string(),
                    done,
                    total,
                    percent: (overall * 100.0) as u8,
                    seconds_left: left.max(0.0) as u64,
                },
            );
        }
    };

    let profile = {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|e| e.to_string())?;
        crate::calibrate::active_profile(&db)
    };

    let res_dir = app.path().resource_dir().ok();
    let kind = model.unwrap_or_else(|| "base".to_string());
    let (target_model, binary) = resolve_model_and_binary(res_dir.as_deref(), &kind)?;

    // Finding what was read aloud does not need the good model — it needs the fast
    // one, because it has to listen to every second of every sermon. The settings are
    // then compared on the model the operator will actually use.
    let (scout_model, _) = resolve_model_and_binary(res_dir.as_deref(), "base")
        .unwrap_or_else(|_| (target_model.clone(), binary.clone()));

    // The scripture lookups are the only thing that touches the database, so hand the
    // pass a closure that locks only for each brief lookup — the hours of decoding in
    // between never hold the lock, and the console stays responsive throughout. The
    // very same `learn::run` powers the offline `learn_cli`, so a baked profile is
    // identical to one learned here.
    let learned = {
        let state = app.state::<AppState>();
        let reading = |text: &str| state.db.lock().ok().and_then(|db| learn::reading_of(&db, text));
        // The wizard is the operator asking for this speaker to be learned from these
        // recordings, so it applies what it finds; the incumbent is scored only to
        // report how the settings in force did on the same audio.
        let incumbent = state
            .db
            .lock()
            .ok()
            .map(|db| crate::calibrate::load(&db, &target_model, &profile));
        // The operator is sitting in front of the wizard waiting for it; it runs to the end.
        learn::run(&scout_model, &target_model, &binary, &paths, incumbent, || true, reading, say)?
    };

    {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|e| e.to_string())?;
        // Go in through the same door as every other change to a speaker, so what the
        // wizard replaces is kept and "Undo last change" can put it back. A wizard run
        // that makes a speaker worse is otherwise unrecoverable for a guest, who has no
        // shipped version to fall back on.
        let model_file = target_model
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| crate::flavor::model_file(&kind));
        let mut next = crate::profile_seed::capture(&db, &profile);
        next.decode.insert(
            model_file,
            crate::profile_seed::DecodeSeed::from_decode(&learned.decode),
        );
        next.room = Some(learned.room.speech_above);
        if let Some(code) = &learned.translation {
            next.translation = Some(code.clone());
        }
        // Names are added to what this speaker already had, never replacing it: the
        // manglings learned on earlier recordings are still true of them.
        for (word, osis) in &learned.aliases {
            next.aliases.entry(word.clone()).or_insert_with(|| osis.clone());
        }
        crate::relearn::install(&db, &profile, &next).map_err(|e| e.to_string())?;
        // In force straight away, without waiting for the next listen.
        crate::books::set_learned_names(learn::book_names(&db, &profile));
    }

    Ok(crate::learn::LearnResult {
        profile,
        recordings: learned.recordings,
        minutes: learned.minutes,
        translation: learned.translation.clone(),
        speech_above: learned.room.speech_above,
        references_found: learned.references_found,
        before: learned.before,
        after: learned.after,
        settings: crate::calibrate::label_of(&learned.decode),
        learned_names: learned.aliases.iter().map(|(w, _)| w.clone()).collect(),
    })
}

#[tauri::command]
pub fn calibration_script() -> Vec<crate::calibrate::ScriptLine> {
    crate::calibrate::script()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceProfiles {
    pub active: String,
    pub all: Vec<String>,
}

/// Who is preaching today, and who else the app knows. Settings are per speaker, so
/// calibrating a guest never disturbs the regular preacher's tuning.
#[tauri::command]
pub fn voice_profiles(state: tauri::State<'_, AppState>) -> Result<VoiceProfiles, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    Ok(VoiceProfiles { active: crate::calibrate::active_profile(&db), all: crate::calibrate::profiles(&db) })
}

/// Switch to (or create) a speaker. A new name starts on the shipped defaults.
#[tauri::command]
pub fn set_voice_profile(name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the voice a name.".into());
    }
    // A speaker's settings are stored under keys built as `alias:<name>:<word>`, so a
    // colon in the name would let one speaker's learned words be read as another's.
    if name.contains(':') {
        return Err("A speaker's name can't contain a colon — use a dash instead.".into());
    }
    let db = state.db.lock().map_err(|e| e.to_string())?;
    crate::calibrate::set_active_profile(&db, name).map_err(|e| e.to_string())?;
    // Recording is armed for a person, not for the app. A guest stepping up must never
    // inherit the consent the regular preacher gave.
    if !crate::sessions::consented(&db, name) {
        state.recording.store(false, Ordering::SeqCst);
    }
    Ok(())
}

/// Remove an added speaker. The President and Vice-President are baked in and protected.
/// Removing a speaker takes everything of theirs with it — their recordings above all.
/// A church that deletes a guest preacher has every right to expect the recordings of
/// that guest to be gone, not orphaned in a folder nothing points at any more.
#[tauri::command]
pub fn remove_voice_profile(
    app: tauri::AppHandle,
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let name = name.trim();
    if crate::calibrate::is_protected(name) {
        return Err("The President and Vice-President profiles can't be removed.".into());
    }
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let dir = crate::sessions::dir_for(&base, name);
    let _ = crate::sessions::forget(&db, &dir, name);
    let _ = crate::relearn::drop_proposal(&db, name);
    let _ = crate::profile_seed::clear(&db, name);
    crate::calibrate::remove_profile(&db, name).map_err(|e| e.to_string())?;
    // Removing the active speaker falls the app back to the President. Recording was
    // armed for the person who just left, so it must not carry over to whoever the app
    // lands on — being recorded is something each speaker agrees to for themselves.
    state.recording.store(false, Ordering::SeqCst);
    Ok(())
}

/// Record one line of the script: waits for the speaker, endpoints on silence
/// exactly as live listening does. Async, and the work runs off the main thread —
/// a sync command would hold the UI thread for the length of the utterance and
/// freeze the window.
#[tauri::command]
pub async fn record_calibration_line(
    app: tauri::AppHandle,
    index: usize,
) -> Result<CalibrationClip, String> {
    if state_is_listening(&app) {
        return Err("Stop listening before calibrating.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        // Calibrate through the same input the service will use, or the tuning is for
        // a signal that never occurs.
        let audio = crate::audio::record_one_utterance(20, &selected_input(&app)?)?
            .ok_or("Didn't hear anything — check the input and try again.")?;
        let dir = speaker_dir(&app)?;
        let path = dir.join(format!("calib_{index:02}.wav"));
        crate::stt::write_wav_16k_mono(&path, &audio)?;
        Ok(CalibrationClip { index, seconds: audio.len() as f32 / 16_000.0 })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationClip {
    pub index: usize,
    pub seconds: f32,
}

fn state_is_listening(app: &tauri::AppHandle) -> bool {
    app.state::<AppState>().listening.load(Ordering::SeqCst)
}

/// Where this speaker's calibration recordings live. Per speaker, so a guest
/// reading the script on a Sunday cannot overwrite the regular preacher's clips.
fn speaker_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let who = {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|e| e.to_string())?;
        crate::calibrate::active_profile(&db)
    };
    let slug: String = who
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let dir = crate::capture::dir(app).ok_or("no place to store the recording")?.join(slug);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Replay the recorded lines through every candidate setting, score each on
/// whether the right scripture resolved, and keep the winner for this model.
///
/// Minutes of decoding, so it runs off the main thread; progress is streamed as
/// each setting finishes rather than leaving the operator staring at a frozen
/// window.
#[tauri::command]
pub async fn run_calibration(
    app: tauri::AppHandle,
    model: Option<String>,
) -> Result<crate::calibrate::CalibrationResult, String> {
    tauri::async_runtime::spawn_blocking(move || calibration_sweep(app, model))
        .await
        .map_err(|e| e.to_string())?
}

fn calibration_sweep(
    app: tauri::AppHandle,
    model: Option<String>,
) -> Result<crate::calibrate::CalibrationResult, String> {
    use crate::calibrate;
    use std::time::Instant;

    let kind = model.unwrap_or_else(|| "base".to_string());
    let res_dir = app.path().resource_dir().ok();
    let (model_path, binary) = resolve_model_and_binary(res_dir.as_deref(), &kind)?;

    let dir = speaker_dir(&app)?;
    let clips: Vec<(usize, Vec<f32>)> = (0..calibrate::SCRIPT.len())
        .filter_map(|i| {
            let path = dir.join(format!("calib_{i:02}.wav"));
            read_wav_16k(&path).ok().map(|audio| (i, audio))
        })
        .collect();
    if clips.is_empty() {
        return Err("No recordings yet for this speaker.".into());
    }

    let baseline_cfg = crate::stt::Decode::for_model(&model_path);
    let mut scores = Vec::new();
    let mut baseline: Option<calibrate::ConfigScore> = None;

    for cfg in calibrate::candidates() {
        let mut resolved = 0usize;
        let mut secs = 0f32;
        for (i, audio) in &clips {
            let t0 = Instant::now();
            let text = match crate::stt::transcribe(audio, &model_path, &binary, cfg) {
                Ok(t) => crate::corrections::correct(&t),
                Err(_) => continue,
            };
            secs += t0.elapsed().as_secs_f32();
            if calibrate::resolves_to(&text, calibrate::SCRIPT[*i].1) {
                resolved += 1;
            }
        }
        let score = calibrate::ConfigScore {
            label: calibrate::label_of(&cfg),
            resolved,
            total: clips.len(),
            seconds_per_clip: secs / clips.len() as f32,
        };
        let _ = app.emit("calibration-progress", score.clone());
        if cfg == baseline_cfg {
            baseline = Some(score.clone());
        }
        scores.push((cfg, score));
    }

    // Most scripture resolved wins; ties go to the faster setting.
    scores.sort_by(|a, b| {
        b.1.resolved
            .cmp(&a.1.resolved)
            .then(a.1.seconds_per_clip.total_cmp(&b.1.seconds_per_clip))
    });
    let (best_cfg, best) = scores.first().cloned().ok_or("nothing to score")?;

    {
        let db = app.state::<AppState>();
        let db = db.db.lock().map_err(|e| e.to_string())?;
        let who = calibrate::active_profile(&db);
        calibrate::save(&db, &model_path, &who, &best_cfg).map_err(|e| e.to_string())?;
    }

    Ok(crate::calibrate::CalibrationResult {
        baseline: baseline.unwrap_or_else(|| best.clone()),
        best,
        all: scores.into_iter().map(|(_, s)| s).collect(),
    })
}

fn read_wav_16k(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    reader
        .samples::<i16>()
        .map(|s| s.map(|v| v as f32 / i16::MAX as f32).map_err(|e| e.to_string()))
        .collect()
}

/// Start the LAN phone remote; returns every address a phone might reach it on,
/// best guess first.
#[tauri::command]
pub fn start_remote(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    crate::remote::start(app, state.remote_running.clone())
}

// ---- Media library ----

#[tauri::command]
pub fn list_media(app: tauri::AppHandle) -> Vec<crate::media::MediaItem> {
    crate::media::list(&app)
}

/// Add files to the library. Anything that is not an image or a video this can
/// show is skipped rather than stored, because a library row that cannot be
/// projected is only a trap for later.
#[tauri::command]
pub fn add_media(
    app: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<Vec<crate::media::MediaItem>, String> {
    let mut added = 0usize;
    {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|e| e.to_string())?;
        for path in &paths {
            let Some(kind) = crate::media::kind_of(path) else { continue };
            // Standalone files belong to no deck.
            db.add_media(path, &crate::media::title_of(path), kind, "")
                .map_err(|e| e.to_string())?;
            // The picker reaches drives the asset scope does not, so a file is
            // allowed the moment the operator chooses it.
            crate::media::allow_path(&app, path);
            added += 1;
        }
    }
    if added == 0 && !paths.is_empty() {
        return Err("None of those files are images or videos this can show.".into());
    }
    Ok(crate::media::list(&app))
}

#[tauri::command]
pub fn remove_media(app: tauri::AppHandle, id: i64) -> Result<Vec<crate::media::MediaItem>, String> {
    {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.remove_media(id).map_err(|e| e.to_string())?;
    }
    Ok(crate::media::list(&app))
}

#[tauri::command]
pub fn rename_media(
    app: tauri::AppHandle,
    id: i64,
    title: String,
) -> Result<Vec<crate::media::MediaItem>, String> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err("A title cannot be empty.".into());
    }
    {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.rename_media(id, &title).map_err(|e| e.to_string())?;
    }
    Ok(crate::media::list(&app))
}

#[tauri::command]
pub fn move_media(
    app: tauri::AppHandle,
    id: i64,
    up: bool,
) -> Result<Vec<crate::media::MediaItem>, String> {
    {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.move_media(id, up).map_err(|e| e.to_string())?;
    }
    Ok(crate::media::list(&app))
}

/// Which office suites on this machine can convert a deck, best first. Shown to
/// the operator so "PowerPoint import doesn't work" is answerable at the desk
/// rather than by guesswork.
#[tauri::command]
pub fn list_converters(app: tauri::AppHandle) -> Vec<(String, String)> {
    crate::media::converters(&app)
        .into_iter()
        // A bare program name is a hopeful PATH lookup, not a found install, so
        // it is not worth reporting as one.
        .filter(|c| c.program.is_absolute())
        .map(|c| (c.name, c.program.to_string_lossy().to_string()))
        .collect()
}

/// Point the app at a converter it did not find. An empty path clears it.
#[tauri::command]
pub fn set_converter(app: tauri::AppHandle, path: String) -> Result<Vec<(String, String)>, String> {
    if !path.trim().is_empty() && crate::media::style_for(std::path::Path::new(path.trim())).is_none()
    {
        return Err(
            "That program is not one this app knows how to drive. Choose soffice (LibreOffice \
             or OpenOffice) or x2t (ONLYOFFICE)."
                .into(),
        );
    }
    crate::media::set_converter_override(&app, &path)?;
    Ok(list_converters(app))
}

/// Turn a PowerPoint or OpenDocument deck into a PDF we can render, and return
/// its path. A PDF is handed straight back, so callers need not care which the
/// operator chose.
#[tauri::command]
pub fn deck_as_pdf(app: tauri::AppHandle, path: String) -> Result<String, String> {
    if !crate::media::needs_conversion(&path) {
        crate::media::allow_path(&app, &path);
        return Ok(path);
    }
    crate::media::convert_to_pdf(&app, &path)
}

/// Store one rendered PDF/deck page as a library image. Called once per page so
/// a long deck reports progress and a failure names the page that failed.
#[tauri::command]
pub fn import_slide(
    app: tauri::AppHandle,
    deck: String,
    index: u32,
    png_base64: String,
) -> Result<crate::media::MediaItem, String> {
    crate::media::save_slide(&app, &deck, index, &png_base64)
}

#[tauri::command]
pub fn project_media(app: tauri::AppHandle, id: i64) -> Result<crate::media::MediaItem, String> {
    crate::media::present(&app, id)
}

/// Transport for the video already on screen. All three flags travel together
/// because they are one picture of how it should be playing.
#[tauri::command]
pub fn set_video_playback(
    app: tauri::AppHandle,
    paused: bool,
    muted: bool,
    looping: bool,
) -> Result<(), String> {
    let next = {
        let state = app.state::<AppState>();
        let cur = state.current.lock().map_err(|e| e.to_string())?;
        match &*cur {
            ProjectionState::Video { src, title, .. } => ProjectionState::Video {
                src: src.clone(),
                title: title.clone(),
                paused,
                muted,
                looping,
            },
            _ => return Err("No video is on screen.".into()),
        }
    };
    project_via_handle(&app, next)
}

/// Jump the live video to a position. An instant rather than a condition, so it
/// travels as its own event instead of living in the projection state.
#[tauri::command]
pub fn seek_video(app: tauri::AppHandle, position_ms: i64) -> Result<(), String> {
    app.emit_to("projection", "video-seek", position_ms.max(0))
        .map_err(|e| e.to_string())
}

/// Step to the previous/next page of the deck the given page belongs to.
#[tauri::command]
pub fn step_deck(
    app: tauri::AppHandle,
    id: i64,
    forward: bool,
) -> Option<crate::media::MediaItem> {
    crate::media::step_deck(&app, id, forward)
}

/// The projection window reporting that the live video reached its end, so an
/// announcements loop can move on at the clip's own length.
#[tauri::command]
pub fn video_ended(state: tauri::State<'_, AppState>) {
    state.video_ended.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn slideshow_running(state: tauri::State<'_, AppState>) -> bool {
    state.slideshow.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn start_slideshow(app: tauri::AppHandle, seconds: u64, looping: bool) -> Result<(), String> {
    let running = app.state::<AppState>().slideshow.clone();
    crate::media::start_slideshow(app.clone(), running, seconds, looping)
}

#[tauri::command]
pub fn stop_slideshow(app: tauri::AppHandle) {
    let running = app.state::<AppState>().slideshow.clone();
    crate::media::stop_slideshow(&app, &running);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Presenting a reference is three things at once: put it on the wall, move
    /// the cursor, and tell the desktop. The LAN remote used to do only the
    /// first, so the phone changed the screen while "Presenting" and the
    /// next/previous buttons stayed on the verse before it. A real end-to-end
    /// check needs a running Tauri app, so this guards the structure instead:
    /// the remote must go through the shared path rather than looking a verse up
    /// and projecting it on its own.
    ///
    /// The same reasoning covers everything else the phone can now put on the
    /// wall. A song slide projected round the side of `project_slide` would skip
    /// the stage monitor and the CCLI usage log; a deck stepped by hand would
    /// leave the console's idea of the live page behind.
    #[test]
    fn the_lan_remote_presents_through_the_shared_path() {
        let remote = include_str!("remote_api.rs");
        for (call, why) in [
            ("present_reference_handle", "references"),
            ("commands::project_slide", "song slides"),
            ("media::step_deck", "deck pages"),
            ("commands::set_video_playback", "video transport"),
            ("commands::set_stage_message", "notes to the stage"),
            ("commands::set_font_scale", "text size"),
        ] {
            assert!(
                remote.contains(call),
                "the remote should present {why} through the shared path, not by hand"
            );
        }
        assert!(
            !remote.contains("do_lookup("),
            "the remote looks a verse up itself again, which skips the cursor and the desktop"
        );
    }

    #[test]
    fn parses_and_strips_spoken_translation() {
        assert_eq!(parse_translation_code("John 3:16 in ASV", false), Some("ASV"));
        assert_eq!(parse_translation_code("give me the King James Romans 8", false), Some("KJV"));
        assert_eq!(parse_translation_code("world english bible please", false), Some("WEB"));
        assert_eq!(parse_translation_code("John 3:16", false), None);

        assert_eq!(strip_translation_phrase("John 3:16 in ASV").trim(), "John 3:16");
        assert_eq!(strip_translation_phrase("the King James John 3:16").trim(), "John 3:16");
        assert_eq!(strip_translation_phrase("Romans 8:28").trim(), "Romans 8:28");
    }

    #[test]
    fn recognizes_all_translations_and_specificity() {
        assert_eq!(parse_translation_code("John 3:16 in the NIV", false), Some("NIV"));
        assert_eq!(parse_translation_code("read it in the New Living Translation", false), Some("NLT"));
        assert_eq!(parse_translation_code("the Berean Standard Bible", false), Some("BSB"));
        assert_eq!(parse_translation_code("in the amplified bible", false), Some("AMP"));
        assert_eq!(parse_translation_code("from the ESV", false), Some("ESV"));
        assert_eq!(parse_translation_code("the Douay Rheims", false), Some("DRB"));

        // Specificity: "new king james" → NKJV, not KJV; "new american standard" → NASB.
        assert_eq!(parse_translation_code("John 3:16 in the New King James Version", false), Some("NKJV"));
        assert_eq!(parse_translation_code("read the New American Standard Bible", false), Some("NASB"));
    }

    #[test]
    fn spelled_out_and_word_safe_abbreviations() {
        // Pastor spells the letters → whisper "N I V" / "N.I.V." → still resolves.
        assert_eq!(parse_translation_code("give it to me in the N I V", false), Some("NIV"));
        assert_eq!(parse_translation_code("John 3:16 N.I.V.", false), Some("NIV"));
        assert_eq!(parse_translation_code("read it in the E S V please", false), Some("ESV"));
        assert_eq!(parse_translation_code("the N K J V", false), Some("NKJV"));
        // Spelled everyday-word abbreviation is unambiguous when spelled out.
        assert_eq!(parse_translation_code("give it in the N E T", false), Some("NET"));

        // As a lone word, an everyday-word abbrev only counts near a scripture.
        assert_eq!(parse_translation_code("they cast the net into the sea", false), None);
        assert_eq!(parse_translation_code("turn up the amp", false), None);
        assert_eq!(parse_translation_code("John 3:16 NET", true), Some("NET"));

        // "amplified" / "amplified version" is a word too — a translation only
        // when a scripture is present; "amplified bible" is always distinctive.
        assert_eq!(parse_translation_code("Matthew 7:7 from the amplified version", true), Some("AMP"));
        assert_eq!(parse_translation_code("in the amplified bible", false), Some("AMP"));
        assert_eq!(parse_translation_code("the amplified guitar was loud", false), None);

        // Stripping removes spelled abbreviations and names for a clean lookup.
        assert_eq!(strip_translation_phrase("John 3:16 in the N I V").trim(), "John 3:16");
        assert_eq!(strip_translation_phrase("Romans 8:28 New King James Version").trim(), "Romans 8:28");
        assert_eq!(strip_translation_phrase("Genesis 1:1 berean standard bible").trim(), "Genesis 1:1");
    }

    fn state_with(translations: &[(&str, &str)], active: &str) -> AppState {
        let db = crate::db::open_in_memory().unwrap();
        db.migrate().unwrap();
        for (code, _name) in translations {
            let json = format!(
                r#"{{"translation":{{"code":"{code}","name":"{code} Bible"}},"verses":[{{"book_osis":"Gen","chapter":1,"verse":1,"text":"{code} Genesis 1:1"}}]}}"#
            );
            db.seed_from_json(&json).unwrap();
        }
        AppState {
            db: Mutex::new(db),
            translation: Mutex::new(active.to_string()),
            current: Mutex::new(ProjectionState::Blank),
            settings: Mutex::new(ProjectionSettings::default()),
            stage: Mutex::new(StageInfo::default()),
            alert: Mutex::new(crate::events::Alert::default()),
            audio: Mutex::new(crate::events::AudioState::default()),
            ready: Arc::new(AtomicBool::new(true)),
            listening: Arc::new(AtomicBool::new(false)),
            recording: Arc::new(AtomicBool::new(false)),
            remote_running: Arc::new(AtomicBool::new(false)),
            slideshow: Arc::new(AtomicBool::new(false)),
            video_ended: Arc::new(AtomicBool::new(false)),
            cursor: Mutex::new(None),
            learned: Mutex::new(std::collections::HashMap::new()),
            moments: Mutex::new(Vec::new()),
            learning: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn resolve_translation_switches_to_installed_translation() {
        let state = state_with(&[("KJV", "kjv"), ("GNT", "gnt")], "KJV");
        // Spoken: reference present + "Good News Bible" → switch to GNT.
        let tr = resolve_translation(&state, "turn to Genesis chapter 1 verse 1 from the Good News Bible");
        assert_eq!(tr, "GNT");
        assert_eq!(state.active_translation(), "GNT");
    }

    #[test]
    fn resolve_translation_falls_back_when_not_installed() {
        // GNT recognized but NOT installed → keep the active translation.
        let state = state_with(&[("KJV", "kjv")], "KJV");
        let tr = resolve_translation(&state, "Genesis 1:1 from the Good News Bible");
        assert_eq!(tr, "KJV");
    }

    #[test]
    fn recognizes_added_popular_translations() {
        assert_eq!(parse_translation_code("Matthew 7:7 in the Good News Bible", false), Some("GNT"));
        assert_eq!(parse_translation_code("the Good News Translation", false), Some("GNTD"));
        assert_eq!(parse_translation_code("read it in the RSV", false), Some("RSV"));
        assert_eq!(parse_translation_code("the Complete Jewish Bible", false), Some("CJB"));
        assert_eq!(parse_translation_code("in the Tree of Life Version", false), Some("TLV"));
        assert_eq!(parse_translation_code("the Contemporary English Version", false), Some("CEVD"));
        assert_eq!(parse_translation_code("give me the M E V", false), Some("MEV"));

        // Specificity: "new revised standard" → NRSV(CE), not RSV.
        assert_eq!(parse_translation_code("the New Revised Standard Version", false), Some("NRSVCE"));

        // Word-like phrases only count near a scripture.
        assert_eq!(parse_translation_code("preaching the good news of the kingdom", false), None);
        assert_eq!(parse_translation_code("John 3:16 good news", true), Some("GNT"));
        assert_eq!(parse_translation_code("the tree of life in the garden of eden", false), None);
        assert_eq!(parse_translation_code("the promise of new life in christ", false), None);
    }
}
