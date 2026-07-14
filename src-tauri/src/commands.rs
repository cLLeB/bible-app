use crate::books::{book_after, book_before, book_by_osis};
use crate::db::{Db, SongSummary};
use crate::events::{ProjectionSettings, ProjectionState, StageInfo, StageSlot, VersePayload};
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
    pub stage: Mutex<StageInfo>,         // what the stage/confidence monitor shows
    // First-run seeding runs on a background thread (it holds the db lock for
    // as long as it takes), so the UI can tell "still installing" from "ready".
    pub ready: Arc<AtomicBool>,
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

    let mut bin_dirs: Vec<PathBuf> = Vec::new();
    if let Some(r) = res_dir {
        bin_dirs.push(r.join("bin"));
    }
    if let Some(root) = &dev_root {
        bin_dirs.push(root.join("bin"));
    }
    let binary = bin_dirs
        .iter()
        .find_map(|d| first_existing(d, &["whisper-cli.exe", "main.exe", "whisper.exe"]))
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
    let res_dir = app.path().resource_dir().ok();
    let (model, binary) = resolve_model_and_binary(res_dir.as_deref(), &kind)?;
    // Use whatever calibration found for this speaker, if they have run it.
    let decode = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        crate::calibrate::load(&db, &model)
    };
    state.listening.store(true, Ordering::SeqCst);
    let flag = state.listening.clone();
    let app2 = app.clone();
    std::thread::spawn(move || crate::audio::run_listen_loop(app2, flag, model, binary, decode));
    Ok(())
}

#[tauri::command]
pub fn stop_listening(state: tauri::State<'_, AppState>) {
    state.listening.store(false, Ordering::SeqCst);
}

// ---- Voice calibration ------------------------------------------------------
// Tune the recognizer to this speaker's voice, mic and room. The clips, the sweep
// and the result all stay on the machine.

#[tauri::command]
pub fn calibration_script() -> Vec<crate::calibrate::ScriptLine> {
    crate::calibrate::script()
}

/// Record one line of the script. Blocks until the speaker finishes (endpointed
/// on silence, exactly as live listening does) or nobody speaks for `wait_secs`.
#[tauri::command]
pub fn record_calibration_line(
    app: tauri::AppHandle,
    index: usize,
) -> Result<CalibrationClip, String> {
    if state_is_listening(&app) {
        return Err("Stop listening before calibrating.".into());
    }
    let audio = crate::audio::record_one_utterance(20)?
        .ok_or("Didn't hear anything — check the microphone and try again.")?;

    let dir = crate::capture::dir(&app).ok_or("no place to store the recording")?;
    let path = dir.join(format!("calib_{index:02}.wav"));
    crate::stt::write_wav_16k_mono(&path, &audio)?;
    Ok(CalibrationClip { index, seconds: audio.len() as f32 / 16_000.0 })
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

/// Replay the recorded lines through every candidate setting, score each on
/// whether the right scripture resolved, and keep the winner for this model.
#[tauri::command]
pub fn run_calibration(
    app: tauri::AppHandle,
    model: Option<String>,
) -> Result<crate::calibrate::CalibrationResult, String> {
    use crate::calibrate;
    use std::time::Instant;

    let kind = model.unwrap_or_else(|| "base".to_string());
    let res_dir = app.path().resource_dir().ok();
    let (model_path, binary) = resolve_model_and_binary(res_dir.as_deref(), &kind)?;

    let dir = crate::capture::dir(&app).ok_or("no recordings found")?;
    let clips: Vec<(usize, Vec<f32>)> = (0..calibrate::SCRIPT.len())
        .filter_map(|i| {
            let path = dir.join(format!("calib_{i:02}.wav"));
            read_wav_16k(&path).ok().map(|audio| (i, audio))
        })
        .collect();
    if clips.is_empty() {
        return Err("No calibration recordings yet.".into());
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
        calibrate::save(&db, &model_path, &best_cfg).map_err(|e| e.to_string())?;
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
            ready: Arc::new(AtomicBool::new(true)),
            listening: Arc::new(AtomicBool::new(false)),
            remote_running: Arc::new(AtomicBool::new(false)),
            cursor: Mutex::new(None),
            learned: Mutex::new(std::collections::HashMap::new()),
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
