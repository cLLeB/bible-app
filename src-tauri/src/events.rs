use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VersePayload {
    pub reference: String,
    pub book: String,
    pub book_osis: String,
    pub chapter: u16,
    pub verse: u16,
    pub text: String,
    pub translation: String,
}

/// A detected verse suggestion for the operator, with a confidence score.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub verse: VersePayload,
    pub confidence: f32,
    pub source: String, // "explicit" | "fuzzy" | "context"
}

/// What the projection window should currently display. Unified across
/// content types (spec §5.1 ProjectionState machine, Phase 1 subset).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ProjectionState {
    Blank,
    Blackout,
    Logo,
    Verse { text: String, caption: String },
    Song { text: String, caption: String },
    /// A full-screen image (e.g. a rendered PDF/PowerPoint page). `src` is a
    /// data URL or an asset-protocol path.
    Image { src: String },
    /// A full-screen video. `src` is an absolute file path the projection window
    /// converts to an asset URL; the file is never copied or re-encoded, so a
    /// library of large videos costs nothing but the operator's own disk.
    ///
    /// Transport lives in the state rather than in a side channel: a window that
    /// reopens mid-service reads `paused`/`muted`/`looping` and comes back the
    /// way the operator left it. Seeking is the one exception, since it is an
    /// instant rather than a condition.
    Video {
        src: String,
        title: String,
        paused: bool,
        muted: bool,
        looping: bool,
    },
    /// The same verse in two translations, side by side.
    Parallel {
        primary_text: String,
        primary_code: String,
        secondary_text: String,
        secondary_code: String,
        caption: String,
    },
    Message { text: String },
    Countdown { target_ms: i64, label: String },
}

/// A lower-third alert overlaid on the congregation screen *on top of* whatever
/// is live (a verse/song stays up). Empty `text` = no alert. `until_ms` is an
/// epoch-millis auto-dismiss time; 0 means it stays until cleared.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub text: String,
    pub until_ms: i64,
}

/// One line of content on the stage/confidence monitor: the big text plus its
/// reference/title caption.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StageSlot {
    pub text: String,
    pub caption: String,
}

/// A timer the platform team watches on the stage monitor (never on the wall):
/// count *up* from a start, or *down* to a target. `anchor_ms` is the start
/// (count-up) or target (count-down) epoch-ms; ignored when mode is "off".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StageTimer {
    pub mode: String, // "off" | "countup" | "countdown"
    pub anchor_ms: i64,
}

impl Default for StageTimer {
    fn default() -> Self {
        Self { mode: "off".into(), anchor_ms: 0 }
    }
}

/// What the stage (confidence) monitor shows the platform team: the current
/// live line, a preview of what's next, any private operator message, and a
/// timer. Deliberately independent of `ProjectionState` so blanking/blackout of
/// the congregation screen does not wipe the stage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StageInfo {
    pub current: Option<StageSlot>,
    pub next: Option<StageSlot>,
    pub message: String,
    #[serde(default)]
    pub timer: StageTimer,
}

/// Display appearance applied over any ProjectionState: a global font multiplier
/// plus the fully-resolved active theme. Carrying the resolved theme (not just an
/// id) means the projection window renders straight from this payload without
/// needing the theme library.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionSettings {
    pub font_scale: f32, // 1.0 = default
    pub theme: crate::themes::Theme,
}

impl Default for ProjectionSettings {
    fn default() -> Self {
        Self { font_scale: 1.0, theme: crate::themes::default_theme() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_state_tags_kind() {
        let s = ProjectionState::Verse { text: "t".into(), caption: "c".into() };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"kind\":\"verse\""));
        let blank = serde_json::to_string(&ProjectionState::Blank).unwrap();
        assert_eq!(blank, "{\"kind\":\"blank\"}");
    }

    #[test]
    fn countdown_round_trips_camel_case_fields() {
        // The frontend sends `targetMs`; struct-variant fields must be camelCased
        // too (enum `rename_all` only touches variant names), else deserialization
        // fails and the command silently errors.
        let json = r#"{"kind":"countdown","targetMs":1234,"label":"Starting soon"}"#;
        let parsed: ProjectionState = serde_json::from_str(json).unwrap();
        assert_eq!(
            parsed,
            ProjectionState::Countdown { target_ms: 1234, label: "Starting soon".into() }
        );
        let out = serde_json::to_string(&parsed).unwrap();
        assert!(out.contains("\"targetMs\":1234"), "got {out}");
    }

    #[test]
    fn serializes_camel_case() {
        let p = VersePayload {
            reference: "John 3:16".into(), book: "John".into(), book_osis: "John".into(),
            chapter: 3, verse: 16, text: "For God...".into(), translation: "WEB".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"reference\":\"John 3:16\""));
        assert!(json.contains("\"translation\":\"WEB\""));
    }
}
