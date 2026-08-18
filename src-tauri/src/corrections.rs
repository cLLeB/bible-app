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
        // Zechariah, heard in a real service as these. They are worse than a plain
        // miss: each is close enough to "Zacchaeus" that the story index answered
        // with Luke 19, so asking for Zechariah 4:6 put the tax collector on the
        // wall. The fuzzy book matcher cannot help, because the misheard word is
        // nearer a famous name than it is to the book.
        ("zacchaeria", "zechariah"),
        ("zacchaediah", "zechariah"),
        ("zachariah", "zechariah"),
        ("zechariiah", "zechariah"),
        ("zakariah", "zechariah"),
        ("goliad", "goliath"),
        ("bethsaida", "bethesda"),
    ];
    m.iter().find(|(k, _)| *k == word).map(|(_, v)| *v)
}

#[cfg(test)]
mod zechariah_tests {
    use super::correct;

    /// Heard in a real service. Each of these is nearer "Zacchaeus" than it is to
    /// "Zechariah", so the story index answered with Luke 19 and the tax collector
    /// went on the wall when the preacher asked for Zechariah 4:6.
    #[test]
    fn zechariah_survives_being_misheard_as_zacchaeus() {
        for said in [
            "Zacchaeria, chapter 4 verse 6.",
            "Zacchaediah, chapter 4, verse 6.",
            "zachariah chapter 4 verse 6",
        ] {
            let out = correct(said).to_lowercase();
            assert!(out.contains("zechariah"), "{said} -> {out}");
        }
    }

    /// The real Zacchaeus must still come through, or fixing one name breaks another.
    #[test]
    fn the_actual_zacchaeus_is_left_alone() {
        assert!(correct("Zaccheus climbed the tree").to_lowercase().contains("zacchaeus"));
    }
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
