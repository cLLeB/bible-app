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
    /// Distinctive single words for loose-paraphrase recall. When the speaker
    /// retells a story without any curated phrase ("a man who had two sons who
    /// asked for his inheritance"), enough of these firing still resolves it.
    #[serde(default)]
    keywords: Vec<String>,
}

static PASSAGES: OnceLock<Vec<Passage>> = OnceLock::new();

fn passages() -> &'static Vec<Passage> {
    PASSAGES.get_or_init(|| serde_json::from_str(include_str!("../passages.json")).unwrap_or_default())
}

#[derive(serde::Deserialize)]
struct Topic {
    name: String,
    phrases: Vec<String>,
    refs: Vec<(String, u16, u16)>,
}

static TOPICS: OnceLock<Vec<Topic>> = OnceLock::new();

fn topics() -> &'static Vec<Topic> {
    TOPICS.get_or_init(|| serde_json::from_str(include_str!("../topics.json")).unwrap_or_default())
}

/// A verse coordinate (osis, chapter, verse).
pub type VerseCoord = (String, u16, u16);

type CrossRefs = std::collections::HashMap<String, Vec<VerseCoord>>;
static CROSSREFS: OnceLock<CrossRefs> = OnceLock::new();

fn crossrefs() -> &'static CrossRefs {
    CROSSREFS.get_or_init(|| serde_json::from_str(include_str!("../crossrefs.json")).unwrap_or_default())
}

/// Cross-references ("related verses") for a presented verse, so the speaker can
/// jump to connected passages (Treasury-of-Scripture style). Empty if none.
pub fn related_verses(osis: &str, chapter: u16, verse: u16) -> Vec<(String, u16, u16)> {
    let key = format!("{osis} {chapter}:{verse}");
    crossrefs().get(&key).cloned().unwrap_or_default()
}

/// Cue phrases that signal the speaker wants a topic, not a specific verse:
/// "what does the Bible say about worry", "verses on forgiveness", etc.
const TOPIC_CUES: &[&str] = &[
    "say about", "says about", "said about", "talks about", "talk about",
    "talking about", "verses about", "verses on", "verse about", "scripture about",
    "scriptures about", "scriptures on", "scripture on", "teaches about", "teach about",
    "teaching on", "teaching about", "on the topic of", "on the subject of",
    "concerning", "when it comes to", "the subject of", "the topic of",
];

/// Conservative stemmer so inflected forms match ("worshipping"→"worship",
/// "healed"→"heal"): strip one common suffix, then collapse a trailing doubled
/// consonant left behind ("worshipp"→"worship"). Short words are left alone.
fn stem(word: &str) -> String {
    let w = word.to_lowercase();
    let mut stemmed = w.clone();
    for suf in ["ings", "ing", "edly", "edness", " ment", "ness", "ed", "ers", "er", "es", "ly", "s"] {
        let suf = suf.trim();
        if w.len() > suf.len() + 2 && w.ends_with(suf) {
            stemmed = w[..w.len() - suf.len()].to_string();
            break;
        }
    }
    let bytes = stemmed.as_bytes();
    let n = bytes.len();
    if n >= 2 && bytes[n - 1] == bytes[n - 2] && bytes[n - 1].is_ascii_alphabetic() {
        stemmed.pop();
    }
    stemmed
}

/// Score how strongly a topic matches a span of text. A multi-word phrase
/// ("god's love") is more specific than a bare word, so it scores higher and
/// outranks a generic single-word topic that merely shares a word. Single-word
/// phrases match on their stem so inflected forms count.
fn score_topic(t: &Topic, span: &str, stems: &std::collections::HashSet<String>) -> usize {
    t.phrases
        .iter()
        .map(|p| {
            if p.contains(' ') {
                if span.contains(p.as_str()) { 2 } else { 0 }
            } else if stems.contains(&stem(p)) {
                1
            } else {
                0
            }
        })
        .sum()
}

/// Detect a topical request ("what the Bible says about forgiveness"). Requires
/// a cue phrase, and matches the topic only against the words *after* the cue —
/// that is what the speaker is actually asking for — so the framing words in the
/// cue ("the bible", "scripture") don't themselves count as a topic. Returns the
/// topic name and its representative verses (best-known first).
pub fn detect_topic(text: &str) -> Option<(String, Vec<VerseCoord>)> {
    let lower = text.to_lowercase();
    let mut best: Option<(&Topic, usize)> = None;
    for cue in TOPIC_CUES {
        let Some(pos) = lower.find(cue) else { continue };
        let tail = &lower[pos + cue.len()..];
        if tail.trim().is_empty() {
            continue;
        }
        let stems: std::collections::HashSet<String> = word_set(tail).iter().map(|w| stem(w)).collect();
        for t in topics() {
            let score = score_topic(t, tail, &stems);
            if score > 0 && best.is_none_or(|(_, s)| score > s) {
                best = Some((t, score));
            }
        }
    }
    best.map(|(t, _)| (t.name.clone(), t.refs.clone()))
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
        ("final book of the bible", "Rev"),
        ("final book of the old testament", "Mal"),
        ("final book of the new testament", "Rev"),
        ("opening book of the bible", "Gen"),
        ("the book of beginnings", "Gen"),
        ("the fourth gospel", "John"),
        ("the revelation of john", "Rev"),
        ("the revelation of jesus christ", "Rev"),
        ("song of songs", "Song"),
        ("canticles", "Song"),
        ("the apocalypse", "Rev"),
        ("apocalypse", "Rev"),
        ("acts of the apostles", "Acts"),
        ("qoheleth", "Eccl"),
        ("the preacher", "Eccl"),
        ("the psalter", "Ps"),
        // Character fun-facts / epithets that unambiguously name one book.
        ("the weeping prophet", "Jer"),
        ("the beloved physician", "Luke"),
        ("the disciple whom jesus loved", "John"),
        ("the beloved disciple", "John"),
        ("the book of the twelve", "Hos"),
        ("the longest book of the bible", "Ps"),
        ("the longest book", "Ps"),
        // Ordinal gospels.
        ("the first gospel", "Matt"),
        ("the second gospel", "Mark"),
        ("the third gospel", "Luke"),
        ("the four gospels", "Matt"),
        ("the synoptic gospels", "Matt"),
        // More book epithets — each unambiguously names one book.
        ("the book of beginnings", "Gen"),
        ("the book of origins", "Gen"),
        ("the book of the exodus", "Exod"),
        ("the book of departure", "Exod"),
        ("the second law", "Deut"),
        ("the hymnbook of israel", "Ps"),
        ("the hebrew hymnbook", "Ps"),
        ("the book of praises", "Ps"),
        ("the laments of jeremiah", "Lam"),
        ("jeremiah's lament", "Lam"),
        ("the gospel prophet", "Isa"),
        ("the shepherd prophet", "Amos"),
        ("the herdsman of tekoa", "Amos"),
        ("the shortest book of the old testament", "Obad"),
        ("the runaway prophet", "Jonah"),
        ("the reluctant prophet", "Jonah"),
        ("the last prophet of the old testament", "Mal"),
        ("the gospel to the jews", "Matt"),
        ("the shortest gospel", "Mark"),
        ("the physician's gospel", "Luke"),
        ("the gospel of love", "John"),
        ("the spiritual gospel", "John"),
        ("the acts of the holy spirit", "Acts"),
        ("the proverbs of the new testament", "Jas"),
        // Canonical groupings — resolve to the first book of the group.
        ("the minor prophets", "Hos"),
        ("the major prophets", "Isa"),
        ("the historical books", "Josh"),
        ("the gospels", "Matt"),
        ("the pauline epistles", "Rom"),
        ("the general epistles", "Jas"),
    ]
}

/// Try to match a descriptive book phrase starting at token `i`.
/// Returns (osis, tokens_consumed).
pub fn resolve_descriptive(tokens: &[String], i: usize) -> Option<(&'static str, usize)> {
    let max = 8usize.min(tokens.len().saturating_sub(i));
    for k in (1..=max).rev() {
        let joined = tokens[i..i + k].join(" ");
        if let Some((_, osis)) = descriptive_map().iter().find(|(p, _)| *p == joined) {
            return Some((osis, k));
        }
    }
    None
}

/// Split text into a set of lowercased alphanumeric word tokens.
fn word_set(text: &str) -> std::collections::HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

/// A scored story/passage match. Higher score = stronger match.
#[derive(Debug, Clone)]
pub struct StoryHit {
    pub osis: String,
    pub chapter: u16,
    pub verse: u16,
    pub score: u32,
}

/// Scan transcript text for famous stories/passages, ranked best-first. Two
/// ways to hit:
///
/// 1. an exact curated phrase appears verbatim ("the prodigal son") → score 1000;
/// 2. loose paraphrase — at least `min_keywords` distinct `keywords` fire →
///    score = matched count.
///
/// `min_keywords` = 3 is the safe live-detection floor; the confirm/refine loop
/// lowers it to 2 to cast a wider net once the speaker is actively describing.
pub fn detect_stories_scored(text: &str, min_keywords: usize) -> Vec<StoryHit> {
    let lower = text.to_lowercase();
    let words = word_set(&lower);
    let mut hits: Vec<StoryHit> = Vec::new();
    for p in passages() {
        let exact = p.phrases.iter().any(|phrase| lower.contains(phrase.as_str()));
        let matched = p.keywords.iter().filter(|k| words.contains(k.as_str())).count();
        let score = if exact {
            1000
        } else if matched >= min_keywords {
            matched as u32
        } else {
            0
        };
        if score > 0 {
            hits.push(StoryHit { osis: p.osis.clone(), chapter: p.chapter, verse: p.verse, score });
        }
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score));
    hits
}

/// Scan transcript text for famous stories/passages (safe live floor). Returns
/// (osis, chapter, verse) for each distinct passage.
pub fn detect_stories(text: &str) -> Vec<(String, u16, u16)> {
    detect_stories_scored(text, 3)
        .into_iter()
        .map(|h| (h.osis, h.chapter, h.verse))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_requires_cue_plus_word() {
        // cue + topic word → fires
        let (name, refs) = detect_topic("let's see what the bible says about forgiveness").unwrap();
        assert_eq!(name, "forgiveness");
        assert!(!refs.is_empty());

        // "verses on worry" → the fear-and-anxiety topic
        let (n2, _) = detect_topic("i have some verses on worry for you").unwrap();
        assert_eq!(n2, "fear and anxiety");

        // topic word without a cue must NOT fire (avoids false positives)
        assert!(detect_topic("i love you all so much today").is_none());
        // cue without a known topic word must NOT fire
        assert!(detect_topic("let me talk about my weekend").is_none());
    }

    #[test]
    fn topics_and_passages_json_are_valid() {
        assert!(!topics().is_empty(), "topics.json failed to parse");
        assert!(!passages().is_empty(), "passages.json failed to parse");
        assert!(!crossrefs().is_empty(), "crossrefs.json failed to parse");
    }

    #[test]
    fn topic_matches_inflected_forms_via_stemming() {
        let (a, _) = detect_topic("what does the bible say about worshipping god").unwrap();
        assert_eq!(a, "praise and worship");
        let (b, _) = detect_topic("some verses on being healed").unwrap();
        assert_eq!(b, "healing");
    }

    #[test]
    fn cross_references_resolve_for_popular_verses() {
        let jn = related_verses("John", 3, 16);
        assert!(jn.contains(&("Rom".into(), 5, 8)), "John 3:16 → Romans 5:8 expected");
        // an unmapped verse yields nothing (no crash)
        assert!(related_verses("Obad", 1, 5).is_empty());
    }
}
