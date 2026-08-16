//! The phone remote's HTTP surface: every route the control page and the
//! projection mirror call, plus the JSON they read.
//!
//! Everything here goes through the same `commands` the operator's console
//! calls, never round its side. That is what keeps the two in step: a slide
//! projected from a phone moves the desktop's cursor, logs CCLI usage, and
//! updates the stage monitor exactly as a click at the laptop would.
//!
//! Routes are deliberately small and text-bodied. A phone on a hall's Wi-Fi is
//! the worst network the app ever sees, so a request carries the fewest bytes
//! that can express the instruction, and replies are either a short confirmation
//! or the error to put in front of the operator.

use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};

use crate::commands::{
    navigate_handle, present_parallel_ref_handle, present_reference_handle, project_via_handle,
    AppState,
};
use crate::events::ProjectionState;

/// How long a phone-sent alert stays up before dismissing itself.
const ALERT_SECONDS: i64 = 12;

/// The band the phone's text-size buttons may move the projection through, and
/// the step they move it by. Wide enough to fix a back-row complaint, narrow
/// enough that nobody can accidentally reduce the wall to nothing from the aisle.
const SCALE_MIN: f32 = 0.6;
const SCALE_MAX: f32 = 2.0;
const SCALE_STEP: f32 = 0.1;

/// How much one tap of the phone's louder/quieter moves the music. A tenth is
/// small enough to ride a track under a speaking voice without overshooting.
const VOLUME_STEP: f32 = 0.1;

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

// ---- Read side ----------------------------------------------------------

/// A one-line description of what the congregation is looking at.
pub fn describe(state: &ProjectionState) -> String {
    match state {
        ProjectionState::Verse { caption, .. } | ProjectionState::Song { caption, .. } => {
            format!("On screen: {caption}")
        }
        ProjectionState::Parallel { caption, primary_code, secondary_code, .. } => {
            format!("On screen: {caption} ({primary_code} / {secondary_code})")
        }
        ProjectionState::Image { .. } => "Image".into(),
        ProjectionState::Video { title, paused, .. } => {
            format!("Video: {title}{}", if *paused { " (paused)" } else { "" })
        }
        ProjectionState::Message { text } => format!("Message: {text}"),
        ProjectionState::Countdown { label, .. } => format!("Countdown: {label}"),
        ProjectionState::Logo => "Logo".into(),
        ProjectionState::Blackout => "Blackout".into(),
        ProjectionState::Blank => "Nothing (blank)".into(),
    }
}

/// The short name for a state, used by the page to decide which contextual
/// controls to show. Kept separate from `describe` so the wording of the
/// summary can change without silently changing behaviour.
fn kind_of(state: &ProjectionState) -> &'static str {
    match state {
        ProjectionState::Blank => "blank",
        ProjectionState::Blackout => "blackout",
        ProjectionState::Logo => "logo",
        ProjectionState::Verse { .. } => "verse",
        ProjectionState::Song { .. } => "song",
        ProjectionState::Image { .. } => "image",
        ProjectionState::Video { .. } => "video",
        ProjectionState::Parallel { .. } => "parallel",
        ProjectionState::Message { .. } => "message",
        ProjectionState::Countdown { .. } => "countdown",
    }
}

pub fn projection_json(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    // Bound to a local so the guard drops before `state` does.
    let json = match state.current.lock() {
        Ok(ref g) => serde_json::to_string(&**g).unwrap_or_else(|_| "{\"kind\":\"blank\"}".into()),
        Err(_) => "{\"kind\":\"blank\"}".into(),
    };
    json
}

/// The live path of whatever is on screen, or empty for states that are not a
/// file. Used to work out which library item the operator is looking at.
fn live_src(state: &ProjectionState) -> String {
    match state {
        ProjectionState::Image { src } => src.clone(),
        ProjectionState::Video { src, .. } => src.clone(),
        _ => String::new(),
    }
}

/// Everything the control page needs on every poll: the summary line, the two
/// running flags, and enough about the live state to raise the right contextual
/// controls without a second request.
///
/// The phone takes the laptop's word for all of it, so reloading the page, or
/// somebody acting at the desk, can never leave a button here lying.
pub fn state_json(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let live = state.current.lock().ok().map(|g| g.clone());
    let summary = live.as_ref().map(describe).unwrap_or_else(|| "…".into());
    let kind = live.as_ref().map(kind_of).unwrap_or("blank");

    // Transport is part of the projection state, so the phone's play/pause
    // button can be drawn in the position it is actually in.
    let video = match live.as_ref() {
        Some(ProjectionState::Video { paused, muted, looping, .. }) => {
            serde_json::json!({ "paused": paused, "muted": muted, "looping": looping })
        }
        _ => serde_json::Value::Null,
    };

    // The deck a live page belongs to, matched by path so it survives a rename
    // and works after a restart. Empty for a standalone file: page buttons only
    // make sense inside a document.
    let src = live.as_ref().map(live_src).unwrap_or_default();
    let deck = if src.is_empty() {
        String::new()
    } else {
        crate::media::list(app)
            .into_iter()
            .find(|m| m.path == src)
            .map(|m| m.deck)
            .unwrap_or_default()
    };

    let font_scale = state.settings.lock().map(|s| s.font_scale).unwrap_or(1.0);

    // Sound is reported whatever is on the wall, because that is the point of
    // it: the operator needs to reach the music while a verse holds the screen,
    // which is exactly when the projection state says nothing about it.
    let sound = state.audio.lock().ok().map(|a| a.clone()).unwrap_or_default();
    let audio = if sound.src.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::json!({
            "title": sound.title,
            "paused": sound.paused,
            "looping": sound.looping,
            "volume": sound.volume,
        })
    };

    serde_json::json!({
        "summary": summary,
        "kind": kind,
        "listening": state.listening.load(Ordering::SeqCst),
        "slideshow": state.slideshow.load(Ordering::SeqCst),
        "video": video,
        "audio": audio,
        "deck": deck,
        "fontScale": font_scale,
        "translation": state.active_translation(),
    })
    .to_string()
}

/// What the projection *looks* like: active theme plus font scale. The mirror
/// polls this so a phone or an OBS browser source shows the sepia the operator
/// chose, not a hardcoded black screen.
pub fn appearance_json(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    // Bound to a local so the guard drops before `state` does.
    let json = match state.settings.lock() {
        Ok(ref s) => serde_json::to_string(&**s).unwrap_or_else(|_| "{}".into()),
        Err(_) => "{}".into(),
    };
    json
}

/// Installed translations plus the active one, so the phone can offer every
/// other translation to compare against, exactly as the console does.
pub fn translations_json(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let active = state.active_translation();
    let list = crate::commands::list_translations(state).unwrap_or_default();
    serde_json::json!({ "active": active, "list": list }).to_string()
}

/// The media library, trimmed to what a phone needs to tap one item.
pub fn media_json(app: &AppHandle) -> String {
    let brief: Vec<_> = crate::media::list(app)
        .into_iter()
        .filter(|m| m.present)
        .map(|m| serde_json::json!({ "id": m.id, "title": m.title, "kind": m.kind }))
        .collect();
    serde_json::to_string(&brief).unwrap_or_else(|_| "[]".into())
}

/// Every song, title only. A church's book can run to hundreds, so the phone
/// filters this list locally rather than asking again on each keystroke.
pub fn songs_json(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let list = crate::commands::list_songs(state).unwrap_or_default();
    let brief: Vec<_> = list
        .into_iter()
        .map(|s| serde_json::json!({ "id": s.id, "title": s.title }))
        .collect();
    serde_json::to_string(&brief).unwrap_or_else(|_| "[]".into())
}

/// How long a slide preview may run on the phone's list. Long enough to tell
/// two verses apart at a glance, short enough that a whole hymn is one small
/// response.
const SLIDE_PREVIEW_CHARS: usize = 70;

fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    text.chars().take(limit).collect::<String>() + "…"
}

/// One song's slides, each with its section label, so the operator can jump
/// straight to the chorus instead of tapping forward to it.
pub fn song_json(app: &AppHandle, id: i64) -> String {
    let state = app.state::<AppState>();
    let slides = crate::commands::get_song_slides(id, state).unwrap_or_default();
    let brief: Vec<_> = slides
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "index": s.order_index,
                "label": s.label.unwrap_or_default(),
                // The wall shows the whole slide; this list only has to be
                // recognisable, and one line of it is.
                "text": clip(&s.text.replace('\n', " "), SLIDE_PREVIEW_CHARS),
            })
        })
        .collect();
    serde_json::to_string(&brief).unwrap_or_else(|_| "[]".into())
}

/// The 66 books in canonical order. Static, so the phone asks once and then
/// walks the whole Bible without touching the network again until it picks a
/// chapter.
pub fn books_json() -> String {
    let brief: Vec<_> = crate::books::BOOKS
        .iter()
        .map(|b| serde_json::json!({ "osis": b.osis, "name": b.name }))
        .collect();
    serde_json::to_string(&brief).unwrap_or_else(|_| "[]".into())
}

/// How many chapters a book has, and how many verses a chapter has, *in the
/// translation the operator is actually using*. Counted from the text rather
/// than from a baked table: a translation with a shorter book must not offer a
/// chapter it does not contain.
///
/// `chapter` of `None` counts chapters in the book; `Some(n)` counts verses in
/// that chapter. Zero means the app has nothing there, which the phone shows as
/// an empty grid rather than a wrong one.
pub fn count_json(app: &AppHandle, book: &str, chapter: Option<u16>) -> String {
    let state = app.state::<AppState>();
    let tr = state.active_translation();
    let count = state
        .db
        .lock()
        .ok()
        .and_then(|db| match chapter {
            Some(c) => db.chapter_last_verse(&tr, book, c).ok().flatten(),
            None => db.book_last_chapter(&tr, book).ok().flatten(),
        })
        .unwrap_or(0);
    serde_json::json!({ "count": count }).to_string()
}

pub fn search_json(app: &AppHandle, query: &str) -> String {
    let state = app.state::<AppState>();
    let hits = crate::commands::search_scripture(query.to_string(), state).unwrap_or_default();
    let brief: Vec<_> = hits
        .into_iter()
        .take(12)
        .map(|v| {
            // The phone shows a one-line preview; sending whole verses over a
            // hall's Wi-Fi for a list of 12 is waste.
            serde_json::json!({ "reference": v.reference, "text": clip(&v.text, 90) })
        })
        .collect();
    serde_json::to_string(&brief).unwrap_or_else(|_| "[]".into())
}

// ---- Routing ------------------------------------------------------------

/// The paths whose replies are JSON. Anything else is a short line of text (or
/// a whole page), and gets labelled accordingly.
pub fn is_json_path(path: &str) -> bool {
    matches!(
        path,
        "/api/projection"
            | "/api/appearance"
            | "/api/translations"
            | "/api/media"
            | "/api/songs"
            | "/api/song"
            | "/api/books"
            | "/api/count"
            | "/api/state"
            | "/api/search"
    )
}

/// The value of one query parameter, or empty. Enough for `?id=7`; the remote
/// has no route that needs more, and a full parser would be more surface than
/// this server should carry.
fn query_param(query: &str, key: &str) -> String {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.to_string())
        .unwrap_or_default()
}

/// Split "a|b" into its two halves, both trimmed. A missing separator means the
/// whole body is the first half and the second is empty.
fn split_pair(body: &str) -> (&str, &str) {
    let (a, b) = body.split_once('|').unwrap_or((body, ""));
    (a.trim(), b.trim())
}

/// Answer one request. `path` has no query string; `query` is what followed the
/// `?`, and `body` is the POST payload, already trimmed.
pub fn route(app: &AppHandle, method: &str, path: &str, query: &str, body: &str) -> (u16, String) {
    match (method, path) {
        ("GET", "/") => (200, crate::remote_pages::remote_page()),
        ("GET", "/projection") => (200, crate::remote_pages::projection_page()),

        ("GET", "/api/projection") => (200, projection_json(app)),
        ("GET", "/api/appearance") => (200, appearance_json(app)),
        ("GET", "/api/translations") => (200, translations_json(app)),
        ("GET", "/api/media") => (200, media_json(app)),
        ("GET", "/api/songs") => (200, songs_json(app)),
        ("GET", "/api/song") => match query_param(query, "id").parse::<i64>() {
            Ok(id) => (200, song_json(app, id)),
            Err(_) => (400, "Unknown song.".into()),
        },
        ("GET", "/api/books") => (200, books_json()),

        // `?book=John` counts its chapters; `?book=John&chapter=3` counts that
        // chapter's verses. One route, because the phone asks the same question
        // one level further down each time.
        ("GET", "/api/count") => {
            let book = query_param(query, "book");
            if book.is_empty() {
                return (400, "Which book?".into());
            }
            let chapter = query_param(query, "chapter").parse::<u16>().ok();
            (200, count_json(app, &book, chapter))
        }

        ("GET", "/api/state") => (200, state_json(app)),

        // The operator is at the projector when the preacher steps up, not at
        // the laptop. Listening starts and stops from the phone too.
        ("POST", "/api/listen") => {
            let on = body == "start";
            let result = if on {
                crate::commands::begin_listening(app, None)
            } else {
                app.state::<AppState>().listening.store(false, Ordering::SeqCst);
                Ok(())
            };
            match result {
                Ok(()) => (200, if on { "listening" } else { "stopped" }.to_string()),
                Err(e) => (500, e),
            }
        }

        // Following a preacher through a passage is the common case; typing a
        // fresh reference for every verse is not.
        ("POST", "/api/nav") => match body {
            "next-verse" | "prev-verse" | "next-chapter" | "prev-chapter" => {
                match navigate_handle(app, body, crate::commands::BY_REMOTE) {
                    Some(v) => (200, v.reference),
                    None => (409, "Nothing on screen to move from.".into()),
                }
            }
            _ => (400, "Unknown direction.".into()),
        },

        ("POST", "/api/display") => {
            let next = match body {
                "blank" => ProjectionState::Blank,
                "blackout" => ProjectionState::Blackout,
                "logo" => ProjectionState::Logo,
                _ => return (400, "Unknown screen mode.".into()),
            };
            let _ = project_via_handle(app, next);
            (200, "ok".into())
        }

        // Kept for older phones that still have the previous page cached.
        ("POST", "/api/blank") => {
            let _ = project_via_handle(app, ProjectionState::Blank);
            (200, "ok".into())
        }

        // Put one library item on the screen, by id.
        ("POST", "/api/media") => match body.parse::<i64>() {
            Ok(id) => match crate::media::present(app, id) {
                Ok(m) => (200, m.title),
                Err(e) => (400, e),
            },
            Err(_) => (400, "Unknown media item.".into()),
        },

        // "<songId>|<slideIndex>". Goes through the console's own command, so
        // the stage monitor and the CCLI usage log follow the phone.
        ("POST", "/api/song") => {
            let (id, index) = split_pair(body);
            let (Ok(id), Ok(index)) = (id.parse::<i64>(), index.parse::<usize>()) else {
                return (400, "Unknown slide.".into());
            };
            let state = app.state::<AppState>();
            match crate::commands::project_slide(app.clone(), state, id, index) {
                Ok(()) => (200, "ok".into()),
                Err(e) => (400, e),
            }
        }

        // Turn the page of the deck the live page belongs to. The id is worked
        // out here from what is on screen, so the phone never has to track it.
        ("POST", "/api/deck") => {
            let forward = match body {
                "next" => true,
                "prev" => false,
                _ => return (400, "Unknown direction.".into()),
            };
            let state = app.state::<AppState>();
            let src = state.current.lock().ok().map(|g| live_src(&g)).unwrap_or_default();
            let here = crate::media::list(app).into_iter().find(|m| m.path == src);
            let Some(here) = here else {
                return (409, "No page is on screen.".into());
            };
            match crate::media::step_deck(app, here.id, forward) {
                Some(m) => (200, m.title),
                None => (409, if forward { "That is the last page." } else { "That is the first page." }.into()),
            }
        }

        // Video transport. Each instruction toggles from the state the video is
        // actually in, so the phone can never desync the flags by guessing.
        ("POST", "/api/video") => {
            let state = app.state::<AppState>();
            let live = state.current.lock().ok().map(|g| g.clone());
            let Some(ProjectionState::Video { paused, muted, looping, .. }) = live else {
                return (409, "No video is on screen.".into());
            };
            let result = match body {
                "pause" => crate::commands::set_video_playback(app.clone(), !paused, muted, looping),
                "mute" => crate::commands::set_video_playback(app.clone(), paused, !muted, looping),
                "loop" => crate::commands::set_video_playback(app.clone(), paused, muted, !looping),
                "restart" => crate::commands::seek_video(app.clone(), 0),
                _ => return (400, "Unknown transport control.".into()),
            };
            match result {
                Ok(()) => (200, "ok".into()),
                Err(e) => (400, e),
            }
        }

        // Sound transport. Reachable whatever is on the wall, which is what
        // separates it from the video controls above: the operator turning the
        // offering music down is not looking at the screen at all.
        ("POST", "/api/audio") => {
            let state = app.state::<AppState>();
            let cur = crate::commands::get_audio(state);
            if cur.src.is_empty() {
                return (409, "No sound is loaded.".into());
            }
            let result = match body {
                "pause" => {
                    crate::commands::set_audio_playback(app.clone(), !cur.paused, cur.looping, cur.volume)
                }
                "loop" => {
                    crate::commands::set_audio_playback(app.clone(), cur.paused, !cur.looping, cur.volume)
                }
                "louder" => crate::commands::set_audio_playback(
                    app.clone(),
                    cur.paused,
                    cur.looping,
                    (cur.volume + VOLUME_STEP).min(1.0),
                ),
                "quieter" => crate::commands::set_audio_playback(
                    app.clone(),
                    cur.paused,
                    cur.looping,
                    (cur.volume - VOLUME_STEP).max(0.0),
                ),
                "restart" => crate::commands::seek_audio(app.clone(), 0),
                "stop" => crate::commands::stop_audio(app.clone()),
                _ => return (400, "Unknown sound control.".into()),
            };
            match result {
                Ok(()) => (200, "ok".into()),
                Err(e) => (400, e),
            }
        }

        // The slideshow runs in the app, not in this page, so the phone starts
        // and stops the same run the console sees.
        ("POST", "/api/slideshow") => {
            let running = app.state::<AppState>().slideshow.clone();
            if body == "stop" {
                crate::media::stop_slideshow(app, &running);
                return (200, "stopped".into());
            }
            match crate::media::start_slideshow(
                app.clone(),
                running,
                crate::media::DEFAULT_SECONDS,
                true,
            ) {
                Ok(()) => (200, "running".into()),
                Err(e) => (400, e),
            }
        }

        ("POST", "/api/search") => {
            if body.is_empty() {
                return (400, "Nothing to search for.".into());
            }
            (200, search_json(app, body))
        }

        ("POST", "/api/alert") => {
            let state = app.state::<AppState>();
            let result = if body.is_empty() {
                crate::commands::clear_alert(app.clone(), state)
            } else {
                crate::commands::show_alert(app.clone(), state, body.to_string(), ALERT_SECONDS)
            };
            match result {
                Ok(()) => (200, "ok".into()),
                Err(e) => (500, e),
            }
        }

        // A full-screen announcement. An empty body clears it back to blank
        // rather than leaving stale words on the wall.
        ("POST", "/api/message") => {
            let next = if body.is_empty() {
                ProjectionState::Blank
            } else {
                ProjectionState::Message { text: body.to_string() }
            };
            let _ = project_via_handle(app, next);
            (200, "ok".into())
        }

        // "<minutes>|<label>". Counting down to the start of a service is the
        // one thing more often set from the door than from the desk.
        ("POST", "/api/countdown") => {
            let (minutes, label) = split_pair(body);
            let Ok(minutes) = minutes.parse::<i64>() else {
                return (400, "How many minutes?".into());
            };
            if minutes <= 0 {
                return (400, "How many minutes?".into());
            }
            let next = ProjectionState::Countdown {
                target_ms: now_ms() + minutes * 60_000,
                label: label.to_string(),
            };
            let _ = project_via_handle(app, next);
            (200, "ok".into())
        }

        // The stage monitor: a private word to whoever is up front. Never
        // reaches the congregation screen, which is the whole point of it.
        ("POST", "/api/stage-note") => {
            let state = app.state::<AppState>();
            match crate::commands::set_stage_message(app.clone(), state, body.to_string()) {
                Ok(()) => (200, "ok".into()),
                Err(e) => (500, e),
            }
        }

        // "<mode>|<seconds>" where mode is countup | countdown | off.
        ("POST", "/api/stage-timer") => {
            let (mode, seconds) = split_pair(body);
            if !matches!(mode, "countup" | "countdown" | "off") {
                return (400, "Unknown timer mode.".into());
            }
            let seconds = seconds.parse::<i64>().unwrap_or(0);
            let state = app.state::<AppState>();
            match crate::commands::set_stage_timer(app.clone(), state, mode.to_string(), seconds) {
                Ok(()) => (200, "ok".into()),
                Err(e) => (500, e),
            }
        }

        // "Make it bigger" is a back-of-the-room judgement, so it is a
        // back-of-the-room control. Steps from whatever the scale is now.
        ("POST", "/api/fontscale") => {
            let state = app.state::<AppState>();
            let current = state.settings.lock().map(|s| s.font_scale).unwrap_or(1.0);
            let next = match body {
                "up" => (current + SCALE_STEP).min(SCALE_MAX),
                "down" => (current - SCALE_STEP).max(SCALE_MIN),
                "reset" => 1.0,
                _ => return (400, "Unknown size step.".into()),
            };
            // Rounded so repeated steps stay on tidy tenths rather than
            // accumulating float dust the operator then sees in the console.
            let next = (next * 10.0).round() / 10.0;
            match crate::commands::set_font_scale(app.clone(), state, next) {
                Ok(()) => (200, format!("{next}")),
                Err(e) => (500, e),
            }
        }

        ("POST", "/api/translation") => {
            if body.is_empty() {
                return (400, "Pick a translation.".into());
            }
            let state = app.state::<AppState>();
            crate::commands::set_translation(body.to_string(), state);
            (200, body.to_string())
        }

        // Goes through the same path as presenting at the laptop, so the cursor
        // and the desktop's "Presenting" follow the phone. Otherwise the next
        // tap on next/previous steps from whatever was up before.
        ("POST", "/api/project") => match present_reference_handle(app, body) {
            Ok(v) => (200, v.reference),
            Err(e) => (400, e),
        },

        // "<code>|<reference>". The reference is optional: left empty it means
        // the verse already on screen, which is what the operator wants after
        // stepping through a passage.
        ("POST", "/api/parallel") => {
            let (code, reference) = split_pair(body);
            if code.is_empty() {
                return (400, "Pick a translation to compare with.".into());
            }
            match present_parallel_ref_handle(app, code, reference) {
                Ok(v) => (200, v.reference),
                Err(e) => (400, e),
            }
        }

        _ => (404, "not found".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_parameter_is_read_and_a_missing_one_is_empty() {
        assert_eq!(query_param("id=7", "id"), "7");
        assert_eq!(query_param("x=1&id=42&y=2", "id"), "42");
        assert_eq!(query_param("", "id"), "");
        assert_eq!(query_param("other=3", "id"), "");
        // A malformed pair must not panic or be mistaken for a value.
        assert_eq!(query_param("id", "id"), "");
    }

    #[test]
    fn a_pair_body_splits_and_trims_and_survives_a_missing_half() {
        assert_eq!(split_pair("12|3"), ("12", "3"));
        assert_eq!(split_pair(" kjv | John 3:16 "), ("kjv", "John 3:16"));
        // The compare control sends only a code when the reference is implied.
        assert_eq!(split_pair("kjv"), ("kjv", ""));
        assert_eq!(split_pair(""), ("", ""));
    }

    #[test]
    fn a_slide_preview_is_one_short_line() {
        let long = "a".repeat(200);
        let out = clip(&long, SLIDE_PREVIEW_CHARS);
        assert_eq!(out.chars().count(), SLIDE_PREVIEW_CHARS + 1, "the ellipsis is the extra char");
        assert!(out.ends_with('…'));
        // Short text is left exactly as it is, with no ellipsis to explain.
        assert_eq!(clip("Amazing grace", SLIDE_PREVIEW_CHARS), "Amazing grace");
        // Multi-byte text must be cut on character boundaries, not bytes.
        let accented = "é".repeat(200);
        assert_eq!(clip(&accented, 10).chars().count(), 11);
    }

    #[test]
    fn every_state_has_a_kind_the_page_can_switch_on() {
        // The contextual controls are chosen by this string, so a new projection
        // state that falls through to a wrong kind would raise wrong buttons.
        assert_eq!(kind_of(&ProjectionState::Blank), "blank");
        assert_eq!(
            kind_of(&ProjectionState::Video {
                src: "a.mp4".into(),
                title: "A".into(),
                paused: false,
                muted: true,
                looping: false,
            }),
            "video"
        );
        assert_eq!(kind_of(&ProjectionState::Image { src: "p.png".into() }), "image");
        assert_eq!(
            kind_of(&ProjectionState::Song { text: "t".into(), caption: "c".into() }),
            "song"
        );
    }

    #[test]
    fn only_the_json_routes_are_labelled_json() {
        assert!(is_json_path("/api/songs"));
        assert!(is_json_path("/api/song"));
        assert!(is_json_path("/api/state"));
        assert!(is_json_path("/api/books"));
        assert!(is_json_path("/api/count"));
        // Routes that answer with a confirmation line must not claim to be JSON,
        // or a phone parsing the reply gets a syntax error instead of the text.
        assert!(!is_json_path("/api/deck"));
        assert!(!is_json_path("/api/video"));
        assert!(!is_json_path("/api/fontscale"));
        assert!(!is_json_path("/"));
    }

    #[test]
    fn the_whole_canon_is_offered_to_the_browser() {
        // The phone builds its book grid from this, so a short list would leave
        // part of the Bible unreachable by tapping.
        let json = books_json();
        let books: serde_json::Value = serde_json::from_str(&json).unwrap();
        let books = books.as_array().unwrap();
        assert_eq!(books.len(), 66, "the browser must offer every book");
        assert_eq!(books[0]["name"], "Genesis", "canonical order, not alphabetical");
        assert_eq!(books[65]["name"], "Revelation");
        // Both fields are needed: osis to ask for counts, name to build the
        // reference the parser reads back.
        assert!(books.iter().all(|b| b["osis"].is_string() && b["name"].is_string()));
    }

    #[test]
    fn the_size_band_cannot_be_stepped_out_of() {
        // Whatever the phone taps, the wall stays inside a range that is still a
        // service. Mirrors the arithmetic in the fontscale route.
        let step = |cur: f32, up: bool| {
            let n = if up { (cur + SCALE_STEP).min(SCALE_MAX) } else { (cur - SCALE_STEP).max(SCALE_MIN) };
            (n * 10.0).round() / 10.0
        };
        assert_eq!(step(SCALE_MAX, true), SCALE_MAX);
        assert_eq!(step(SCALE_MIN, false), SCALE_MIN);
        assert_eq!(step(1.0, true), 1.1);
        assert_eq!(step(1.0, false), 0.9);
    }
}
