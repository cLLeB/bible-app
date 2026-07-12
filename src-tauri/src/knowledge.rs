//! Deep Bible knowledge for creative references: descriptive book names
//! ("last book of the Old Testament", "third book of Moses") and famous
//! stories/passages ("the prodigal son" → Luke 15:11).

use std::sync::OnceLock;

#[derive(serde::Deserialize)]
struct Passage {
    osis: String,
    chapter: u16,
    verse: u16,
    phrases: Vec<String>,
}

static PASSAGES: OnceLock<Vec<Passage>> = OnceLock::new();

fn passages() -> &'static Vec<Passage> {
    PASSAGES.get_or_init(|| serde_json::from_str(include_str!("../passages.json")).unwrap_or_default())
}

/// Descriptive names that don't contain the plain book word.
fn descriptive_map() -> &'static [(&'static str, &'static str)] {
    &[
        ("first book of moses", "Gen"),
        ("second book of moses", "Exod"),
        ("third book of moses", "Lev"),
        ("fourth book of moses", "Num"),
        ("fifth book of moses", "Deut"),
        ("books of moses", "Gen"),
        ("book of moses", "Gen"),
        ("the pentateuch", "Gen"),
        ("pentateuch", "Gen"),
        ("the torah", "Gen"),
        ("torah", "Gen"),
        ("first book of the bible", "Gen"),
        ("last book of the bible", "Rev"),
        ("first book of the old testament", "Gen"),
        ("last book of the old testament", "Mal"),
        ("first book of the new testament", "Matt"),
        ("last book of the new testament", "Rev"),
        ("song of songs", "Song"),
        ("canticles", "Song"),
        ("the apocalypse", "Rev"),
        ("apocalypse", "Rev"),
        ("acts of the apostles", "Acts"),
        ("qoheleth", "Eccl"),
        ("the preacher", "Eccl"),
        ("the psalter", "Ps"),
    ]
}

/// Try to match a descriptive book phrase starting at token `i`.
/// Returns (osis, tokens_consumed).
pub fn resolve_descriptive(tokens: &[String], i: usize) -> Option<(&'static str, usize)> {
    let max = 6usize.min(tokens.len().saturating_sub(i));
    for k in (1..=max).rev() {
        let joined = tokens[i..i + k].join(" ");
        if let Some((_, osis)) = descriptive_map().iter().find(|(p, _)| *p == joined) {
            return Some((osis, k));
        }
    }
    None
}

/// Scan transcript text for famous stories/passages. Returns (osis, chapter,
/// verse) for each distinct passage whose phrase appears.
pub fn detect_stories(text: &str) -> Vec<(String, u16, u16)> {
    let lower = text.to_lowercase();
    let mut hits: Vec<(String, u16, u16)> = Vec::new();
    for p in passages() {
        if p.phrases.iter().any(|phrase| lower.contains(phrase.as_str())) {
            hits.push((p.osis.clone(), p.chapter, p.verse));
        }
    }
    hits
}
