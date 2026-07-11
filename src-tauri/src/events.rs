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

#[cfg(test)]
mod tests {
    use super::*;
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
