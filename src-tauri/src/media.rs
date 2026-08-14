//! The media library and the slideshow that walks it.
//!
//! Two decisions shape this module.
//!
//! **The library holds references, not copies.** A church's media folder is
//! measured in gigabytes; importing would mean a second copy, a slow import, and
//! a library that silently rots when someone tidies the original folder. Adding
//! a file records its path. A file that has since moved is reported as missing
//! rather than pretended about.
//!
//! **The slideshow runs in the backend.** It has to keep advancing while the
//! operator switches console tabs, opens the theme editor, or walks away from
//! the laptop entirely, and it has to be controllable from the phone. A timer
//! living in a React component satisfies none of that: the console learned this
//! the hard way when the stage monitor stopped following the phone because the
//! component holding the listener was on the other tab.

use crate::commands::AppState;
use crate::events::{ProjectionState, StageSlot};
use base64::Engine;
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Extensions the projection window can actually show. Kept in step with
/// `src/lib/media.ts`, which offers them in the file picker.
const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp", "avif"];
const VIDEO_EXT: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv", "avi"];

/// How long a slideshow holds each item, and the bounds a typed value is held
/// to. The floor exists because a mistyped 0 would otherwise flash the whole
/// library past the congregation in a second.
pub const MIN_SECONDS: u64 = 2;
pub const MAX_SECONDS: u64 = 600;
pub const DEFAULT_SECONDS: u64 = 8;

/// How often the runner wakes to notice a stop. Short enough that Stop feels
/// immediate, long enough that an idle slideshow costs nothing.
const TICK: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub kind: String,
    /// The document this page came from, empty for a standalone file. Pages of
    /// one deck stay together, and stepping moves inside it.
    pub deck: String,
    /// False when the file is no longer where it was added from. The operator
    /// finds out in the library, not when it fails to appear on the wall.
    pub present: bool,
}

/// What kind of media a path is, or None when it is not something we can show.
pub fn kind_of(path: &str) -> Option<&'static str> {
    let ext = Path::new(path).extension()?.to_str()?.to_lowercase();
    if IMAGE_EXT.contains(&ext.as_str()) {
        return Some("image");
    }
    if VIDEO_EXT.contains(&ext.as_str()) {
        return Some("video");
    }
    None
}

/// The file's own name, without directories or extension, as a starting title.
pub fn title_of(path: &str) -> String {
    let stem = Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if stem.trim().is_empty() {
        Path::new(path).file_name().and_then(|s| s.to_str()).unwrap_or(path).to_string()
    } else {
        stem.to_string()
    }
}

/// A dwell time from whatever arrived, held to the bounds above.
pub fn clamp_seconds(seconds: u64) -> u64 {
    if seconds == 0 {
        return DEFAULT_SECONDS;
    }
    seconds.clamp(MIN_SECONDS, MAX_SECONDS)
}

/// The next index, or None when the run is over. Looping never ends, so an
/// announcements loop can be left running before a service starts.
pub fn next_index(current: usize, count: usize, looping: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let next = current + 1;
    if next < count {
        Some(next)
    } else if looping {
        Some(0)
    } else {
        None
    }
}

/// The projection state for one library item. Videos start playing, unmuted and
/// not looping: a bumper that silently loops forever is the wrong default, and
/// the operator can set either from the controls.
pub fn state_for(path: &str, title: &str, kind: &str) -> ProjectionState {
    if kind == "video" {
        ProjectionState::Video {
            src: path.to_string(),
            title: title.to_string(),
            paused: false,
            muted: false,
            looping: false,
        }
    } else {
        ProjectionState::Image { src: path.to_string() }
    }
}

/// A file-system-safe folder name for a deck, so two imports of "Sunday.pdf"
/// and "sunday .pdf" cannot collide or escape the slides folder.
pub fn slug(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let trimmed = cleaned.trim_matches('-').to_string();
    let short: String = trimmed.chars().take(60).collect();
    if short.is_empty() {
        "deck".into()
    } else {
        short
    }
}

/// Where rendered slide pages live: inside the app's own data directory, so a
/// deck imported on Saturday is still there on Sunday and survives a restart.
/// The previous approach kept pages only as data URLs in a React component,
/// which meant re-importing before every service and pushing megabytes of text
/// through the event channel on every single page change.
pub fn slides_dir(app: &AppHandle, deck: &str) -> Result<std::path::PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("slides")
        .join(slug(deck));
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    Ok(base)
}

/// Let the projection window actually load this file.
///
/// The asset protocol is scoped, and the configured scope covers the user's own
/// folders. Church media does not live there: it lives on a second drive or a
/// mapped NAS share, which the picker will happily add and the webview would
/// then refuse to load, showing black. Rather than open the scope to the whole
/// disk, each file the operator deliberately adds is allowed by name.
///
/// Failures are ignored on purpose: a path that cannot be allowed will surface
/// as that item failing to display, which is information the operator can act
/// on, where a startup error about a file they have forgotten about is not.
pub fn allow_path(app: &AppHandle, path: &str) {
    let _ = app.asset_protocol_scope().allow_file(path);
}

/// Re-allow everything the library already holds. The scope is per-run, so
/// without this a file added last Sunday is refused after the next restart.
pub fn allow_known_paths(app: &AppHandle) {
    for item in list(app) {
        allow_path(app, &item.path);
    }
    // Theme backgrounds are chosen from the same kind of folder and were
    // scoped the same way.
    let state = app.state::<AppState>();
    // Bound to a local so the guard drops before `state` does.
    let src = match state.settings.lock() {
        Ok(ref s) => s.theme.background.src.clone(),
        Err(_) => String::new(),
    };
    if !src.trim().is_empty() {
        allow_path(app, &src);
    }
}

/// Write one rendered page to disk and put it in the library, so a deck's pages
/// are ordinary media: previewable, projectable, orderable, and usable as
/// service cues or slideshow items like anything else.
pub fn save_slide(
    app: &AppHandle,
    deck: &str,
    index: u32,
    encoded: &str,
) -> Result<MediaItem, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|_| "That page could not be decoded.".to_string())?;
    let dir = slides_dir(app, deck)?;
    // Zero-padded so a 100-page deck still sorts as a human reads it.
    let path = dir.join(format!("page-{index:03}.png"));
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    let path_str = path.to_string_lossy().to_string();
    let title = format!("{} · {}", deck.trim(), index);
    {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.add_media(&path_str, &title, "image", deck.trim()).map_err(|e| e.to_string())?;
    }
    allow_path(app, &path_str);
    list(app)
        .into_iter()
        .find(|m| m.path == path_str)
        .ok_or_else(|| "The page was written but not listed.".to_string())
}

pub fn list(app: &AppHandle) -> Vec<MediaItem> {
    let state = app.state::<AppState>();
    let rows = match state.db.lock() {
        Ok(db) => db.list_media().unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    rows.into_iter()
        .map(|(id, path, title, kind, deck)| MediaItem {
            present: Path::new(&path).exists(),
            id,
            path,
            title,
            kind,
            deck,
        })
        .collect()
}

/// Move to the previous or next page of the same deck, and show it.
///
/// This is what stepping through a deck means, and it is a different thing from
/// the announcements loop: next and previous stay inside the document being
/// presented rather than wandering into whatever else is in the library.
/// Returns None at either end of the deck, or for a standalone file.
pub fn step_deck(app: &AppHandle, id: i64, forward: bool) -> Option<MediaItem> {
    let items = list(app);
    let current = items.iter().find(|m| m.id == id)?;
    if current.deck.is_empty() {
        return None;
    }
    let pages: Vec<&MediaItem> = items.iter().filter(|m| m.deck == current.deck).collect();
    let at = pages.iter().position(|m| m.id == id)?;
    let target = if forward { at.checked_add(1)? } else { at.checked_sub(1)? };
    let wanted = pages.get(target)?.id;
    present(app, wanted).ok()
}

/// Put one library item on the screen, and name it on the stage monitor with
/// whatever follows it, so the platform team sees what is coming.
pub fn present(app: &AppHandle, id: i64) -> Result<MediaItem, String> {
    let items = list(app);
    let pos = items
        .iter()
        .position(|m| m.id == id)
        .ok_or_else(|| "That item is no longer in the library.".to_string())?;
    present_at(app, &items, pos)
}

fn present_at(app: &AppHandle, items: &[MediaItem], pos: usize) -> Result<MediaItem, String> {
    let item = items.get(pos).ok_or_else(|| "Nothing to show.".to_string())?;
    if !item.present {
        return Err(format!("'{}' is no longer at {}", item.title, item.path));
    }
    crate::commands::project_via_handle(app, state_for(&item.path, &item.title, &item.kind))?;
    let next = items.get(pos + 1).map(|n| StageSlot {
        text: n.title.clone(),
        caption: kind_label(&n.kind).to_string(),
    });
    crate::commands::set_stage_handle(
        app,
        Some(StageSlot { text: item.title.clone(), caption: kind_label(&item.kind).to_string() }),
        next,
    );
    Ok(item.clone())
}

fn kind_label(kind: &str) -> &'static str {
    if kind == "video" {
        "Video"
    } else {
        "Image"
    }
}

/// Longest a loop will wait on a video before giving up on hearing that it
/// ended. A file the projection window cannot decode would otherwise hold the
/// loop forever, which on a Sunday morning looks exactly like a crash.
const VIDEO_PATIENCE: Duration = Duration::from_secs(60 * 20);

/// Start the announcements loop. Idempotent: starting a running loop is a no-op
/// rather than a second thread racing the first for the screen.
///
/// A timer is the wrong unit for a video, so the loop does not use one there: a
/// video is held until it reports that it ended, then the loop moves on. The
/// dwell time governs images, which have no natural length of their own.
pub fn start_slideshow(
    app: AppHandle,
    running: Arc<AtomicBool>,
    seconds: u64,
    looping: bool,
) -> Result<(), String> {
    let items = list(&app);
    if items.iter().all(|m| !m.present) {
        return Err("The media library has nothing to show.".into());
    }
    if running.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let dwell = clamp_seconds(seconds);
    let _ = app.emit("slideshow-changed", true);

    std::thread::spawn(move || {
        let mut pos = 0usize;
        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }
            // Re-read every step: the operator may add, rename or remove
            // something while it runs, and the next slide should be the library
            // as it is now, not as it was when Start was pressed.
            let items = list(&app);
            if items.is_empty() {
                break;
            }
            if pos >= items.len() {
                pos = 0;
            }
            // Skip anything that has gone missing rather than stopping dead.
            let shown = present_at(&app, &items, pos);
            if shown.is_err() {
                match next_index(pos, items.len(), looping) {
                    Some(n) if n != 0 || looping => {
                        pos = n;
                        continue;
                    }
                    _ => break,
                }
            }
            let is_video = shown.map(|m| m.kind == "video").unwrap_or(false);
            let held = if is_video {
                wait_for_video(&app, &running)
            } else {
                sleep_unless_stopped(&running, dwell)
            };
            if !held {
                break;
            }
            match next_index(pos, items.len(), looping) {
                Some(n) => pos = n,
                None => break,
            }
        }
        running.store(false, Ordering::SeqCst);
        let _ = app.emit("slideshow-changed", false);
    });
    Ok(())
}

/// Hold until the projection window says the video ended, the operator stops,
/// or patience runs out. False when stopped.
fn wait_for_video(app: &AppHandle, running: &Arc<AtomicBool>) -> bool {
    let ended = app.state::<AppState>().video_ended.clone();
    ended.store(false, Ordering::SeqCst);
    let deadline = std::time::Instant::now() + VIDEO_PATIENCE;
    loop {
        if !running.load(Ordering::SeqCst) {
            return false;
        }
        if ended.load(Ordering::SeqCst) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return true;
        }
        std::thread::sleep(TICK);
    }
}

/// Sleep `seconds`, waking often enough to notice a stop. False when stopped.
fn sleep_unless_stopped(running: &Arc<AtomicBool>, seconds: u64) -> bool {
    let ticks = (seconds * 1000) / TICK.as_millis() as u64;
    for _ in 0..ticks {
        if !running.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(TICK);
    }
    running.load(Ordering::SeqCst)
}

pub fn stop_slideshow(app: &AppHandle, running: &Arc<AtomicBool>) {
    running.store(false, Ordering::SeqCst);
    let _ = app.emit("slideshow-changed", false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_what_the_screen_can_show() {
        assert_eq!(kind_of("C:/media/backdrop.JPG"), Some("image"));
        assert_eq!(kind_of("/srv/loop.webm"), Some("video"));
        assert_eq!(kind_of("notes.pdf"), None);
        assert_eq!(kind_of("song.mp3"), None);
        assert_eq!(kind_of("README"), None);
    }

    #[test]
    fn the_console_and_the_backend_agree_on_what_media_is() {
        // A file the picker offers but the backend refuses is a split that only
        // shows up mid-service, so the two lists are checked against each other.
        let ts = include_str!("../../src/lib/media.ts");
        for ext in IMAGE_EXT.iter().chain(VIDEO_EXT.iter()) {
            assert!(ts.contains(&format!("\"{ext}\"")), "{ext} is missing from src/lib/media.ts");
        }
    }

    #[test]
    fn deck_folders_cannot_collide_or_escape() {
        assert_eq!(slug("Sunday Morning"), "sunday-morning");
        // Two decks that differ only by punctuation must not share a folder in
        // a way that lets one overwrite the other's pages.
        assert_ne!(slug("Sunday Morning"), slug("SundayMorning"));
        // Nothing that could climb out of the slides directory survives.
        for hostile in ["../../etc/passwd", "..\\..\\windows", "C:/Windows/system32"] {
            let s = slug(hostile);
            assert!(!s.contains('/') && !s.contains('\\') && !s.contains(".."), "got {s}");
        }
        assert_eq!(slug(""), "deck");
        assert_eq!(slug("///"), "deck");
        assert!(slug(&"x".repeat(500)).len() <= 60);
    }

    #[test]
    fn titles_start_from_the_file_name() {
        assert_eq!(title_of("C:\\church\\Advent Week 1.mp4"), "Advent Week 1");
        assert_eq!(title_of("/srv/media/offering.png"), "offering");
    }

    #[test]
    fn a_mistyped_dwell_cannot_flash_the_library_past_the_room() {
        assert_eq!(clamp_seconds(0), DEFAULT_SECONDS);
        assert_eq!(clamp_seconds(1), MIN_SECONDS);
        assert_eq!(clamp_seconds(8), 8);
        assert_eq!(clamp_seconds(99_999), MAX_SECONDS);
    }

    #[test]
    fn walks_forward_and_wraps_only_when_looping() {
        assert_eq!(next_index(0, 3, false), Some(1));
        assert_eq!(next_index(2, 3, false), None);
        assert_eq!(next_index(2, 3, true), Some(0));
        assert_eq!(next_index(0, 0, true), None);
    }

    #[test]
    fn video_starts_playing_and_images_are_plain() {
        match state_for("/m/bumper.mp4", "Bumper", "video") {
            ProjectionState::Video { paused, looping, muted, title, .. } => {
                assert!(!paused, "a video the operator just chose should play");
                assert!(!looping, "looping forever is not a safe default for a bumper");
                assert!(!muted);
                assert_eq!(title, "Bumper");
            }
            other => panic!("expected a video, got {other:?}"),
        }
        assert_eq!(
            state_for("/m/slide.png", "Slide", "image"),
            ProjectionState::Image { src: "/m/slide.png".into() }
        );
    }

    #[test]
    fn library_rows_survive_add_reorder_and_remove() {
        let db = crate::db::open_in_memory().unwrap();
        db.migrate().unwrap();
        let a = db.add_media("/m/a.png", "A", "image", "").unwrap();
        let b = db.add_media("/m/b.mp4", "B", "video", "").unwrap();

        // Adding the same folder again must not multiply what is there.
        assert_eq!(db.add_media("/m/a.png", "A", "image", "").unwrap(), a);
        assert_eq!(db.list_media().unwrap().len(), 2);

        assert!(db.move_media(b, true).unwrap(), "B should move up");
        let order: Vec<i64> = db.list_media().unwrap().into_iter().map(|(id, ..)| id).collect();
        assert_eq!(order, vec![b, a]);

        // The ends of the list have nothing to swap with.
        assert!(!db.move_media(b, true).unwrap());
        assert!(!db.move_media(a, false).unwrap());

        db.rename_media(a, "Offering").unwrap();
        assert_eq!(db.media_at(a).unwrap().unwrap().1, "Offering");

        db.remove_media(a).unwrap();
        assert_eq!(db.list_media().unwrap().len(), 1);
        assert!(db.media_at(a).unwrap().is_none());
    }

    #[test]
    fn deck_pages_stay_grouped_and_standalone_files_are_loose() {
        // Stepping through a deck must not wander into the rest of the library,
        // which is the whole difference between presenting a document and
        // running an announcements loop.
        let db = crate::db::open_in_memory().unwrap();
        db.migrate().unwrap();
        db.add_media("/s/sermon/page-001.png", "Sermon · 1", "image", "Sermon").unwrap();
        db.add_media("/s/sermon/page-002.png", "Sermon · 2", "image", "Sermon").unwrap();
        db.add_media("/m/loose.png", "Loose", "image", "").unwrap();

        let rows = db.list_media().unwrap();
        let decks: Vec<String> = rows.iter().map(|(.., deck)| deck.clone()).collect();
        assert_eq!(decks, vec!["Sermon", "Sermon", ""]);
        assert_eq!(rows.iter().filter(|(.., d)| d == "Sermon").count(), 2);
    }
}
