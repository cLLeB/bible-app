use std::collections::HashSet;

const STOPWORDS: &[&str] = &[
    "the", "and", "that", "for", "with", "this", "from", "have", "his", "her", "who", "was",
    "are", "but", "not", "you", "your", "they", "them", "will", "unto", "shall", "hath", "were",
    "there", "their", "which", "when", "then", "than", "into", "upon", "what", "would", "could",
];

/// Words too common to distinguish one verse from another.
pub fn is_filler(word: &str) -> bool {
    STOPWORDS.contains(&word)
}

/// Build an FTS5 query from transcript text (significant words OR'd together)
/// plus the word list used to score overlap. None if too few words to trust.
pub fn fts_query(text: &str) -> Option<(String, Vec<String>)> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut words: Vec<String> = Vec::new();
    for raw in text.split_whitespace() {
        let w: String = raw.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
        if w.len() >= 4 && !STOPWORDS.contains(&w.as_str()) && seen.insert(w.clone()) {
            words.push(w);
        }
    }
    words.truncate(10);
    if words.len() < 3 {
        return None;
    }
    let query = words.iter().map(|w| format!("\"{w}\"")).collect::<Vec<_>>().join(" OR ");
    Some((query, words))
}

/// How many query words appear in the verse text.
pub fn overlap(query_words: &[String], verse_text: &str) -> usize {
    let vt = verse_text.to_lowercase();
    query_words.iter().filter(|w| vt.contains(w.as_str())).count()
}

/// True if a match is strong enough to surface as a quote suggestion.
pub fn is_strong(overlap: usize, total: usize) -> bool {
    overlap >= 3 && (overlap as f32 / total.max(1) as f32) >= 0.5
}
