//! Voice calibration: tune the recognizer to *this* speaker, mic and room.
//!
//! Decode settings that are right for one voice are wrong for another — accent,
//! pace, mic distance and room all move the answer, and the weaker models are
//! the most sensitive. Rather than ship one guess for everyone, the operator can
//! read a short script aloud; we then replay their own recordings through every
//! candidate setting and keep whichever one resolves the most scripture.
//!
//! The score is not word-accuracy. A transcript can be wrong in ways the detector
//! shrugs off ("romans eight twenty eight") and right in ways it cannot use, so
//! what gets measured is what matters: did the correct verse come out.
//!
//! Everything stays on the machine: the clips, the sweep and the result.

use crate::db::Db;
use crate::stt::{Decode, Window};
use serde::Serialize;
use std::path::Path;

/// What the operator is asked to say, and the reference it must resolve to.
/// Chosen to cover the ways references actually get spoken: plain, ordinal books
/// ("second Timothy"), hard names, a reference buried in a sentence, and a
/// spoken translation switch.
pub const SCRIPT: &[(&str, &str)] = &[
    ("John chapter 3 verse 16", "John 3:16"),
    ("Romans 8 28", "Romans 8:28"),
    ("Second Timothy chapter 3 verse 16", "2 Timothy 3:16"),
    ("Ephesians 2 verses 8 and 9", "Ephesians 2:8"),
    ("First Corinthians chapter 13", "1 Corinthians 13"),
    ("Psalm 23", "Psalms 23"),
    ("Habakkuk chapter 2 verse 4", "Habakkuk 2:4"),
    ("Nehemiah chapter 8 verse 10", "Nehemiah 8:10"),
    ("Isaiah 40 verse 31", "Isaiah 40:31"),
    ("Turn with me to Matthew chapter 5 from verse 3", "Matthew 5:3"),
    ("Let's look at Revelation chapter 21", "Revelation 21"),
    ("Philippians 4 verse 13", "Philippians 4:13"),
];

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScriptLine {
    pub index: usize,
    pub say: String,
    pub expect: String,
}

pub fn script() -> Vec<ScriptLine> {
    SCRIPT
        .iter()
        .enumerate()
        .map(|(i, (say, expect))| ScriptLine {
            index: i,
            say: (*say).to_string(),
            expect: (*expect).to_string(),
        })
        .collect()
}

/// The settings worth trying. Deliberately few: every extra candidate costs the
/// operator real time reading and waiting.
///
/// The window is swept widest, because it is the axis that moves the answer most
/// — and it moves it in opposite directions per model (base wants the full 30s
/// window, small wants it trimmed to the clip). Margins must reach down to 1.2:
/// on real speech that is where small peaks, and an earlier candidate list that
/// started at 2.0 could not find its own best setting.
pub fn candidates() -> Vec<Decode> {
    let mut out = Vec::new();
    for window in [
        Window::Full,
        Window::Fit { margin: 1.2 },
        Window::Fit { margin: 1.5 },
        Window::Fit { margin: 2.0 },
    ] {
        for beam in [5, 1] {
            out.push(Decode { beam, prompt: true, normalize: true, window });
        }
    }
    // Is the scripture prompt helping this speaker, or dragging them around?
    // (On the voices tested it is load-bearing — dropping it fell to 0-2 of 12 —
    // but that is exactly the kind of thing worth confirming per speaker.)
    out.push(Decode { prompt: false, ..Decode::default() });
    out
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConfigScore {
    pub label: String,
    pub resolved: usize,
    pub total: usize,
    pub seconds_per_clip: f32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationResult {
    pub best: ConfigScore,
    pub all: Vec<ConfigScore>,
    pub baseline: ConfigScore,
}

pub fn label_of(d: &Decode) -> String {
    let win = match d.window {
        Window::Full => "full window".to_string(),
        Window::Fit { margin } => format!("fitted x{margin}"),
    };
    let search = if d.beam > 1 { format!("beam {}", d.beam) } else { "greedy".to_string() };
    let prompt = if d.prompt { "" } else { ", no prompt" };
    format!("{win}, {search}{prompt}")
}

/// Did the detector land on the expected reference for this transcript?
pub fn resolves_to(text: &str, expected: &str) -> bool {
    let Some(want) = crate::reference::parse_reference(expected) else {
        return false;
    };
    let mut ctx = crate::detect::RefContext::default();
    crate::detect::detect_with_context(text, &mut ctx)
        .first()
        .map(|hit| {
            hit.reference.book_osis == want.book_osis
                && hit.reference.chapter == want.chapter
                // A chapter-only expectation accepts any verse within it.
                && want.verse.map(|v| hit.reference.verse == Some(v)).unwrap_or(true)
        })
        .unwrap_or(false)
}

/// The church's own preachers, baked in and always present. Settings are stored per
/// speaker, so learning a guest never disturbs these two. They cannot be removed.
pub const DEFAULT_PROFILES: &[&str] = &["President", "Vice-President"];

/// One of the baked-in preachers, which the operator must not be able to delete.
pub fn is_protected(name: &str) -> bool {
    DEFAULT_PROFILES.iter().any(|p| *p == name)
}

/// Remove an added speaker. The two baked-in preachers are protected. If the one being
/// removed was active, fall back to the President.
pub fn remove_profile(db: &Db, name: &str) -> rusqlite::Result<()> {
    if is_protected(name) {
        return Ok(());
    }
    let mut all = profiles(db);
    all.retain(|p| p != name);
    db.set_setting("voice_profiles", &serde_json::to_string(&all).unwrap_or_default())?;
    if active_profile(db) == name {
        db.set_setting("voice_profile", DEFAULT_PROFILES[0])?;
    }
    Ok(())
}

pub fn profiles(db: &Db) -> Vec<String> {
    let stored = db
        .get_setting("voice_profiles")
        .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
        .unwrap_or_default();
    if stored.is_empty() {
        DEFAULT_PROFILES.iter().map(|s| s.to_string()).collect()
    } else {
        stored
    }
}

pub fn active_profile(db: &Db) -> String {
    db.get_setting("voice_profile")
        .unwrap_or_else(|| DEFAULT_PROFILES[0].to_string())
}

pub fn set_active_profile(db: &Db, name: &str) -> rusqlite::Result<()> {
    let mut all = profiles(db);
    if !all.iter().any(|p| p == name) {
        all.push(name.to_string());
        let json = serde_json::to_string(&all).unwrap_or_default();
        db.set_setting("voice_profiles", &json)?;
    }
    db.set_setting("voice_profile", name)
}

/// Persist the winning settings for this speaker + model, so listening uses them.
pub fn save(db: &Db, model: &Path, profile: &str, d: &Decode) -> rusqlite::Result<()> {
    let window = match d.window {
        Window::Full => "full".to_string(),
        Window::Fit { margin } => margin.to_string(),
    };
    let value = format!("beam={},prompt={},normalize={},window={}", d.beam, d.prompt as u8, d.normalize as u8, window);
    db.set_setting(&key_for(model, profile), &value)
}

/// Load this speaker's calibrated settings for this model, falling back to the
/// shipped defaults — which are themselves measured on real preaching, so an
/// uncalibrated speaker is not starting from nothing.
pub fn load(db: &Db, model: &Path, profile: &str) -> Decode {
    let mut d = Decode::for_model(model);
    let Some(spec) = db.get_setting(&key_for(model, profile)) else { return d };
    for part in spec.split(',') {
        let Some((k, v)) = part.split_once('=') else { continue };
        match k {
            "beam" => d.beam = v.parse().unwrap_or(d.beam),
            "prompt" => d.prompt = v != "0",
            "normalize" => d.normalize = v != "0",
            "window" => {
                d.window = if v == "full" {
                    Window::Full
                } else {
                    v.parse().map(|margin| Window::Fit { margin }).unwrap_or(d.window)
                }
            }
            _ => {}
        }
    }
    d
}

fn key_for(model: &Path, profile: &str) -> String {
    let name = model.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    format!("decode:{name}:{profile}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_script_line_states_a_parseable_reference() {
        for (say, expect) in SCRIPT {
            assert!(
                crate::reference::parse_reference(expect).is_some(),
                "script line '{say}' expects unparseable '{expect}'"
            );
        }
    }

    #[test]
    fn scores_a_transcript_against_the_expected_reference() {
        assert!(resolves_to("John chapter 3 verse 16", "John 3:16"));
        assert!(!resolves_to("John chapter 3 verse 16", "Romans 8:28"));
        // A chapter-only expectation accepts whatever verse the detector picks.
        assert!(resolves_to("First Corinthians chapter 13", "1 Corinthians 13"));
    }

    #[test]
    fn settings_survive_a_save_and_load() {
        let db = crate::db::open_at(Path::new(":memory:")).unwrap();
        db.migrate().unwrap();
        let model = Path::new("ggml-small.en.bin");
        let want = Decode { beam: 1, prompt: false, normalize: true, window: Window::Fit { margin: 2.0 } };
        save(&db, model, "Miss Hilda", &want).unwrap();
        assert_eq!(load(&db, model, "Miss Hilda"), want);
    }

    #[test]
    fn uncalibrated_speakers_fall_back_to_shipped_defaults() {
        let db = crate::db::open_at(Path::new(":memory:")).unwrap();
        db.migrate().unwrap();
        let model = Path::new("ggml-base.en.bin");
        assert_eq!(load(&db, model, "Guest"), Decode::for_model(model));
    }

    /// The whole point: a guest tuning their voice on a Sunday must leave the
    /// regular preacher's tuning exactly as it was.
    #[test]
    fn calibrating_one_speaker_does_not_disturb_another() {
        let db = crate::db::open_at(Path::new(":memory:")).unwrap();
        db.migrate().unwrap();
        let model = Path::new("ggml-base.en.bin");

        let hers = Decode { beam: 5, prompt: true, normalize: true, window: Window::Full };
        let theirs = Decode { beam: 1, prompt: true, normalize: false, window: Window::Fit { margin: 1.5 } };
        save(&db, model, "Miss Hilda", &hers).unwrap();
        save(&db, model, "Guest", &theirs).unwrap();

        assert_eq!(load(&db, model, "Miss Hilda"), hers);
        assert_eq!(load(&db, model, "Guest"), theirs);
    }

    #[test]
    fn the_regular_preacher_is_the_default_voice() {
        let db = crate::db::open_at(Path::new(":memory:")).unwrap();
        db.migrate().unwrap();
        assert_eq!(active_profile(&db), "President");
        assert!(profiles(&db).contains(&"Vice-President".to_string()));

        set_active_profile(&db, "Pastor Visiting").unwrap();
        assert_eq!(active_profile(&db), "Pastor Visiting");
        assert!(profiles(&db).contains(&"Pastor Visiting".to_string()));

        // Added speakers can be removed; the baked-in preachers cannot.
        remove_profile(&db, "Pastor Visiting").unwrap();
        assert!(!profiles(&db).contains(&"Pastor Visiting".to_string()));
        remove_profile(&db, "President").unwrap();
        assert!(profiles(&db).contains(&"President".to_string()), "President is protected");
    }
}
