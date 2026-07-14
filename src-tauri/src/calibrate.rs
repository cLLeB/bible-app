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

/// Persist the winning settings for this model, so listening uses them from now on.
pub fn save(db: &Db, model: &Path, d: &Decode) -> rusqlite::Result<()> {
    let window = match d.window {
        Window::Full => "full".to_string(),
        Window::Fit { margin } => margin.to_string(),
    };
    let value = format!("beam={},prompt={},normalize={},window={}", d.beam, d.prompt as u8, d.normalize as u8, window);
    db.set_setting(&key_for(model), &value)
}

/// Load calibrated settings for this model, falling back to the shipped defaults.
pub fn load(db: &Db, model: &Path) -> Decode {
    let mut d = Decode::for_model(model);
    let Some(spec) = db.get_setting(&key_for(model)) else { return d };
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

fn key_for(model: &Path) -> String {
    let name = model.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    format!("decode:{name}")
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
        save(&db, model, &want).unwrap();
        assert_eq!(load(&db, model), want);
    }

    #[test]
    fn uncalibrated_models_fall_back_to_shipped_defaults() {
        let db = crate::db::open_at(Path::new(":memory:")).unwrap();
        db.migrate().unwrap();
        let model = Path::new("ggml-base.en.bin");
        assert_eq!(load(&db, model), Decode::for_model(model));
    }
}
