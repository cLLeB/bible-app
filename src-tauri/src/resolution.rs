//! The confirm/refine conversation loop.
//!
//! After we present a best-guess verse for a paraphrased/uncertain reference,
//! we keep the alternatives on standby and interpret whatever the speaker says
//! next. The guiding rule (from the speaker's point of view):
//!
//!   * he reads the verse, or affirms it ("yes", "amen", "that's the one") → keep it;
//!   * he denies it ("no", "not this") or keeps describing what he wants → we
//!     search again with everything he has said and present a better guess;
//!   * he simply moves on to other things → we quietly keep the last guess.
//!
//! We treat almost everything he says as usable signal, adapting to his
//! speaking style rather than forcing him to phrase a query our way.

/// A best-guess verse presented to the speaker with its alternatives held on
/// standby, plus everything he has said so far to describe what he wants.
#[derive(Debug, Clone, Default)]
pub struct Pending {
    /// Ranked alternatives; `candidates[0]` is the one currently presented.
    pub candidates: Vec<crate::reference::ParsedRef>,
    /// Accumulated description across the speaker's refining utterances.
    pub description: String,
    /// Text of the presented verse — used to notice when he reads it aloud.
    pub presented_text: String,
    /// Consecutive utterances that carried no usable refinement signal.
    pub misses: u32,
}

/// How a follow-up utterance relates to the verse currently on standby.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    /// Explicit rejection ("no", "not that one").
    Deny,
    /// Explicit acceptance, or the speaker reading the presented verse aloud.
    Affirm,
    /// Anything else — treated as more description to refine on, or, if it
    /// carries no usable signal, as the speaker moving on.
    Other,
}

/// Minimum distinct content words shared with the presented verse to conclude
/// the speaker is reading it aloud (a strong confirmation).
const READ_OVERLAP_MIN: usize = 4;

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

const STOPWORDS: &[&str] = &[
    "the", "and", "that", "for", "with", "this", "from", "have", "his", "her", "who", "was",
    "are", "but", "not", "you", "your", "they", "them", "will", "unto", "shall", "hath", "were",
    "there", "their", "which", "when", "then", "than", "into", "upon", "what", "would", "could",
    "here", "have", "about", "just", "like", "some", "them", "these", "those", "over",
];

fn content_words(text: &str) -> Vec<String> {
    words(text)
        .into_iter()
        .filter(|w| w.len() >= 4 && !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// Whole-word denial signals and multi-word denial phrases.
fn is_denial(text: &str) -> bool {
    let lower = text.to_lowercase();
    const PHRASES: &[&str] = &[
        "not this", "not that", "not it", "not the one", "not quite", "not right",
        "that's not", "thats not", "wrong one", "wrong verse", "another one",
        "a different", "different one", "don't think", "dont think", "no not",
    ];
    if PHRASES.iter().any(|p| lower.contains(p)) {
        return true;
    }
    let ws = words(&lower);
    // A bare "no" / "nope" (whole word, so "now"/"know" don't count).
    ws.iter().any(|w| matches!(w.as_str(), "no" | "nope" | "nah" | "negative"))
}

/// Whole-word acceptance signals and multi-word affirmation phrases.
fn is_affirmation(text: &str) -> bool {
    let lower = text.to_lowercase();
    const PHRASES: &[&str] = &[
        "that's it", "thats it", "that's the one", "thats the one", "there it is",
        "you got it", "that's right", "thats right", "that is it", "that is the one",
        "yes that", "perfect", "exactly right",
    ];
    if PHRASES.iter().any(|p| lower.contains(p)) {
        return true;
    }
    let ws = words(&lower);
    ws.iter().any(|w| {
        matches!(
            w.as_str(),
            "yes" | "yeah" | "yep" | "yup" | "amen" | "correct" | "exactly"
                | "right" | "okay" | "ok" | "absolutely" | "precisely" | "perfect"
        )
    })
}

/// Distinct content words the utterance shares with the presented verse text.
fn reading_overlap(text: &str, presented_verse: &str) -> usize {
    let verse: std::collections::HashSet<String> = content_words(presented_verse).into_iter().collect();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut n = 0;
    for w in content_words(text) {
        if verse.contains(&w) && seen.insert(w) {
            n += 1;
        }
    }
    n
}

/// Classify a follow-up utterance against the verse currently on standby.
/// Denial wins over affirmation (so "no, that's not right" is a Deny), and a
/// clear affirmation or reading of the verse confirms it; everything else is
/// `Other`, which the caller turns into a fresh search or a quiet settle.
pub fn classify(text: &str, presented_verse: &str) -> Response {
    if is_denial(text) {
        return Response::Deny;
    }
    if is_affirmation(text) || reading_overlap(text, presented_verse) >= READ_OVERLAP_MIN {
        return Response::Affirm;
    }
    Response::Other
}

/// A stable signature for a description: its distinct significant words, sorted.
/// Two paraphrases of the same request map to the same signature, so an operator
/// correction can be recalled the next time. Empty if too thin to be reliable.
pub fn signature(text: &str) -> String {
    let mut ws: Vec<String> = content_words(text);
    ws.sort();
    ws.dedup();
    if ws.len() < 2 {
        return String::new();
    }
    ws.join(" ")
}

/// Strip a leading denial word so the remainder ("no, it's the one about the
/// son who came back") can be fed into the refine search as description.
pub fn description_part(text: &str) -> String {
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();
    for lead in ["no ", "nope ", "not this ", "not that ", "no not "] {
        if let Some(rest) = lower.strip_prefix(lead) {
            let start = trimmed.len() - rest.len();
            return trimmed[start..].trim().to_string();
        }
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRODIGAL: &str =
        "A certain man had two sons and the younger of them said give me my inheritance";

    #[test]
    fn denials_are_detected() {
        assert_eq!(classify("no", PRODIGAL), Response::Deny);
        assert_eq!(classify("no that's not the one", PRODIGAL), Response::Deny);
        assert_eq!(classify("not this one", PRODIGAL), Response::Deny);
        assert_eq!(classify("that's not quite right", PRODIGAL), Response::Deny);
        // a denial that also carries new description is still a denial
        assert_eq!(classify("no it's more about the lost sheep", PRODIGAL), Response::Deny);
    }

    #[test]
    fn affirmations_are_detected() {
        assert_eq!(classify("yes that's the one", PRODIGAL), Response::Affirm);
        assert_eq!(classify("amen", PRODIGAL), Response::Affirm);
        assert_eq!(classify("okay", PRODIGAL), Response::Affirm);
        assert_eq!(classify("exactly right there it is", PRODIGAL), Response::Affirm);
    }

    #[test]
    fn reading_the_verse_confirms_it() {
        // Speaker starts reading the presented verse — strong confirmation even
        // without an explicit "yes".
        let read = "a certain man had two sons and the younger asked for his inheritance";
        assert_eq!(classify(read, PRODIGAL), Response::Affirm);
    }

    #[test]
    fn ordinary_words_that_arent_denial_or_affirm_are_other() {
        assert_eq!(classify("it's the one where he ends up feeding the pigs", PRODIGAL), Response::Other);
        assert_eq!(classify("let me describe it a little more", PRODIGAL), Response::Other);
    }

    #[test]
    fn common_words_do_not_trigger_false_denial() {
        // "know" / "now" must not read as "no"
        assert_ne!(classify("now i know the story continues", PRODIGAL), Response::Deny);
    }

    #[test]
    fn description_part_strips_leading_denial() {
        assert_eq!(description_part("no it's the lost sheep"), "it's the lost sheep");
        assert_eq!(description_part("the good samaritan"), "the good samaritan");
    }

    #[test]
    fn signature_is_order_independent_and_stable() {
        // Same significant words in any order → same signature (so a re-phrased
        // request recalls the operator's earlier correction).
        let a = signature("the younger son wasted his inheritance");
        let b = signature("inheritance wasted by the younger son");
        assert_eq!(a, b);
        assert!(!a.is_empty());
        // Too thin to key on.
        assert_eq!(signature("the it"), "");
    }
}
