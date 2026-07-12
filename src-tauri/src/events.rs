use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VersePayload {
    pub reference: String,
    pub book: String,
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
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectionState {
    Blank,
    Verse { text: String, caption: String },
    Song { text: String, caption: String },
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
    fn serializes_camel_case() {
        let p = VersePayload {
            reference: "John 3:16".into(), book: "John".into(), chapter: 3,
            verse: 16, text: "For God...".into(), translation: "WEB".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"reference\":\"John 3:16\""));
        assert!(json.contains("\"translation\":\"WEB\""));
    }
}
