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

pub fn list(app: &AppHandle) -> Vec<MediaItem> {
    let state = app.state::<AppState>();
    let rows = match state.db.lock() {
        Ok(db) => db.list_media().unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    rows.into_iter()
        .map(|(id, path, title, kind)| MediaItem {
            present: Path::new(&path).exists(),
            id,
            path,
            title,
            kind,
        })
        .collect()
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

/// Start walking the library. Idempotent: starting a running slideshow is a
/// no-op rather than a second thread racing the first for the screen.
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
            if present_at(&app, &items, pos).is_err() {
                match next_index(pos, items.len(), looping) {
                    Some(n) if n != 0 || looping => {
                        pos = n;
                        continue;
                    }
                    _ => break,
                }
            }
            if !sleep_unless_stopped(&running, dwell) {
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
        let a = db.add_media("/m/a.png", "A", "image").unwrap();
        let b = db.add_media("/m/b.mp4", "B", "video").unwrap();

        // Adding the same folder again must not multiply what is there.
        assert_eq!(db.add_media("/m/a.png", "A", "image").unwrap(), a);
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
}
