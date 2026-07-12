//! High-precision correction of common speech-to-text mishearings for biblical
//! vocabulary. The fuzzy book matcher already recovers near-miss *book* names
//! (e.g. "galations" → Galatians), so this focuses on what it can't: famous
//! person/place names used in story matching, and a few words whisper renders
//! as a completely different real word ("Philippines" for Philippians).

/// Whole-word corrections, keyed by the lowercased misheard token.
fn correction(word: &str) -> Option<&'static str> {
    let m: &[(&str, &str)] = &[
        // A real word whisper substitutes for a book — fuzzy won't catch it.
        ("philippines", "philippians"),
        ("philippine", "philippians"),
        // Famous names used by the story/paraphrase index.
        ("zaccheus", "zacchaeus"),
        ("zacheus", "zacchaeus"),
        ("nebuchadnezar", "nebuchadnezzar"),
        ("nebuchadnezer", "nebuchadnezzar"),
        ("melchisedek", "melchizedek"),
        ("methusela", "methuselah"),
        ("mephiboseth", "mephibosheth"),
        ("bartimeus", "bartimaeus"),
        ("gethsemani", "gethsemane"),
        ("habakuk", "habakkuk"),
        ("habbakuk", "habakkuk"),
        ("nicodemas", "nicodemus"),
        ("zerubabel", "zerubbabel"),
        ("goliad", "goliath"),
        ("bethsaida", "bethesda"),
    ];
    m.iter().find(|(k, _)| *k == word).map(|(_, v)| *v)
}

/// Apply corrections word-by-word, preserving surrounding text. A token is
/// matched by its lowercased alphanumeric core so trailing punctuation and case
/// don't block a fix.
pub fn correct(text: &str) -> String {
    text.split_whitespace()
        .map(|tok| {
            let core: String = tok.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
            match correction(&core) {
                Some(fixed) => tok.to_lowercase().replace(&core, fixed),
                None => tok.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixes_known_mishearings() {
        assert_eq!(correct("turn to Philippines 4:13"), "turn to philippians 4:13");
        assert_eq!(correct("the story of Zaccheus the tax collector"),
                   "the story of zacchaeus the tax collector");
        assert_eq!(correct("Nebuchadnezar had a dream"), "nebuchadnezzar had a dream");
    }

    #[test]
    fn leaves_ordinary_text_untouched() {
        let s = "and so the lord spoke to the people";
        assert_eq!(correct(s), s);
    }
}
