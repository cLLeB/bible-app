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
        ("zacchaiah", "zechariah"),
        ("zachaiah", "zechariah"),
        ("zecharia", "zechariah"),
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
        assert!(correct("the story of Zacchaeus").to_lowercase().contains("zacchaeus"));
    }

    /// The same word, two meanings, told apart by what comes after it.
    ///
    /// Heard in a service as "Zacchaeus, chapter 4 verse 6" and answered with Luke 19,
    /// because that is who Zacchaeus is. But a person does not have a chapter, so in
    /// front of one the word can only be Zechariah.
    #[test]
    fn a_person_followed_by_a_chapter_is_a_book_instead() {
        for said in [
            "Zacchaeus, chapter 4 verse 6.",
            "Zaccheus chapter 4 verse 6",
            "Zacchaeus 4 verse 6",
        ] {
            let out = correct(said).to_lowercase();
            assert!(out.contains("zechariah"), "{said} -> {out}");
        }
        // And with nothing after it, he is still himself.
        assert!(correct("Zacchaeus").to_lowercase().contains("zacchaeus"));
        assert!(correct("Zacchaeus was a tax collector").to_lowercase().contains("zacchaeus"));
    }
}

/// Names that are a real person in one breath and a misheard book in the next.
///
/// "Zacchaeus" is a person, and the story index rightly answers Luke 19 for him. But
/// "Zacchaeus chapter 4 verse 6" is not about a person at all: nothing in scripture
/// takes a chapter and verse except a book, so in that position the word can only be
/// a mangled Zechariah. Heard exactly that way in a service, and it put the tax
/// collector on the wall when the preacher asked for Zechariah 4:6.
///
/// Applied ONLY when a chapter or verse follows, which is what makes it safe. A
/// blanket rename would break every genuine Zacchaeus reference to buy this one.
fn correction_before_a_reference(word: &str) -> Option<&'static str> {
    let m: &[(&str, &str)] = &[
        ("zacchaeus", "zechariah"),
        ("zaccheus", "zechariah"),
        ("zacheus", "zechariah"),
        ("zachaeus", "zechariah"),
    ];
    m.iter().find(|(k, _)| *k == word).map(|(_, v)| *v)
}

/// Does a chapter/verse reference start at this token? Either the word "chapter", or
/// a bare number as in "Zechariah 4". This is the grammar only a book can precede.
fn starts_reference(tok: Option<&&str>) -> bool {
    let Some(t) = tok else { return false };
    let w: String = t.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    w == "chapter" || w == "chapters" || (!w.is_empty() && w.chars().all(|c| c.is_ascii_digit()))
}

/// Apply corrections, preserving surrounding text. A token is matched by its
/// lowercased alphanumeric core so trailing punctuation and case do not block a fix.
pub fn correct(text: &str) -> String {
    let toks: Vec<&str> = text.split_whitespace().collect();
    toks.iter()
        .enumerate()
        .map(|(i, tok)| {
            let core: String =
                tok.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
            // The position-sensitive fix goes first: it is the more specific claim,
            // and it only fires where the plain reading is impossible.
            let fixed = if starts_reference(toks.get(i + 1)) {
                correction_before_a_reference(&core).or_else(|| correction(&core))
            } else {
                correction(&core)
            };
            match fixed {
                Some(f) => tok.to_lowercase().replace(&core, f),
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
