#[derive(Debug, Clone, Copy)]
pub struct CanonicalBook {
    pub osis: &'static str,
    pub name: &'static str,
    #[allow(dead_code)] // used for canonical ordering in later phases
    pub order: u8,
}

// 66 books, canonical order. (osis, display name, order)
pub static BOOKS: &[CanonicalBook] = &[
    CanonicalBook { osis: "Gen", name: "Genesis", order: 1 },
    CanonicalBook { osis: "Exod", name: "Exodus", order: 2 },
    CanonicalBook { osis: "Lev", name: "Leviticus", order: 3 },
    CanonicalBook { osis: "Num", name: "Numbers", order: 4 },
    CanonicalBook { osis: "Deut", name: "Deuteronomy", order: 5 },
    CanonicalBook { osis: "Josh", name: "Joshua", order: 6 },
    CanonicalBook { osis: "Judg", name: "Judges", order: 7 },
    CanonicalBook { osis: "Ruth", name: "Ruth", order: 8 },
    CanonicalBook { osis: "1Sam", name: "1 Samuel", order: 9 },
    CanonicalBook { osis: "2Sam", name: "2 Samuel", order: 10 },
    CanonicalBook { osis: "1Kgs", name: "1 Kings", order: 11 },
    CanonicalBook { osis: "2Kgs", name: "2 Kings", order: 12 },
    CanonicalBook { osis: "1Chr", name: "1 Chronicles", order: 13 },
    CanonicalBook { osis: "2Chr", name: "2 Chronicles", order: 14 },
    CanonicalBook { osis: "Ezra", name: "Ezra", order: 15 },
    CanonicalBook { osis: "Neh", name: "Nehemiah", order: 16 },
    CanonicalBook { osis: "Esth", name: "Esther", order: 17 },
    CanonicalBook { osis: "Job", name: "Job", order: 18 },
    CanonicalBook { osis: "Ps", name: "Psalms", order: 19 },
    CanonicalBook { osis: "Prov", name: "Proverbs", order: 20 },
    CanonicalBook { osis: "Eccl", name: "Ecclesiastes", order: 21 },
    CanonicalBook { osis: "Song", name: "Song of Solomon", order: 22 },
    CanonicalBook { osis: "Isa", name: "Isaiah", order: 23 },
    CanonicalBook { osis: "Jer", name: "Jeremiah", order: 24 },
    CanonicalBook { osis: "Lam", name: "Lamentations", order: 25 },
    CanonicalBook { osis: "Ezek", name: "Ezekiel", order: 26 },
    CanonicalBook { osis: "Dan", name: "Daniel", order: 27 },
    CanonicalBook { osis: "Hos", name: "Hosea", order: 28 },
    CanonicalBook { osis: "Joel", name: "Joel", order: 29 },
    CanonicalBook { osis: "Amos", name: "Amos", order: 30 },
    CanonicalBook { osis: "Obad", name: "Obadiah", order: 31 },
    CanonicalBook { osis: "Jonah", name: "Jonah", order: 32 },
    CanonicalBook { osis: "Mic", name: "Micah", order: 33 },
    CanonicalBook { osis: "Nah", name: "Nahum", order: 34 },
    CanonicalBook { osis: "Hab", name: "Habakkuk", order: 35 },
    CanonicalBook { osis: "Zeph", name: "Zephaniah", order: 36 },
    CanonicalBook { osis: "Hag", name: "Haggai", order: 37 },
    CanonicalBook { osis: "Zech", name: "Zechariah", order: 38 },
    CanonicalBook { osis: "Mal", name: "Malachi", order: 39 },
    CanonicalBook { osis: "Matt", name: "Matthew", order: 40 },
    CanonicalBook { osis: "Mark", name: "Mark", order: 41 },
    CanonicalBook { osis: "Luke", name: "Luke", order: 42 },
    CanonicalBook { osis: "John", name: "John", order: 43 },
    CanonicalBook { osis: "Acts", name: "Acts", order: 44 },
    CanonicalBook { osis: "Rom", name: "Romans", order: 45 },
    CanonicalBook { osis: "1Cor", name: "1 Corinthians", order: 46 },
    CanonicalBook { osis: "2Cor", name: "2 Corinthians", order: 47 },
    CanonicalBook { osis: "Gal", name: "Galatians", order: 48 },
    CanonicalBook { osis: "Eph", name: "Ephesians", order: 49 },
    CanonicalBook { osis: "Phil", name: "Philippians", order: 50 },
    CanonicalBook { osis: "Col", name: "Colossians", order: 51 },
    CanonicalBook { osis: "1Thess", name: "1 Thessalonians", order: 52 },
    CanonicalBook { osis: "2Thess", name: "2 Thessalonians", order: 53 },
    CanonicalBook { osis: "1Tim", name: "1 Timothy", order: 54 },
    CanonicalBook { osis: "2Tim", name: "2 Timothy", order: 55 },
    CanonicalBook { osis: "Titus", name: "Titus", order: 56 },
    CanonicalBook { osis: "Phlm", name: "Philemon", order: 57 },
    CanonicalBook { osis: "Heb", name: "Hebrews", order: 58 },
    CanonicalBook { osis: "Jas", name: "James", order: 59 },
    CanonicalBook { osis: "1Pet", name: "1 Peter", order: 60 },
    CanonicalBook { osis: "2Pet", name: "2 Peter", order: 61 },
    CanonicalBook { osis: "1John", name: "1 John", order: 62 },
    CanonicalBook { osis: "2John", name: "2 John", order: 63 },
    CanonicalBook { osis: "3John", name: "3 John", order: 64 },
    CanonicalBook { osis: "Jude", name: "Jude", order: 65 },
    CanonicalBook { osis: "Rev", name: "Revelation", order: 66 },
];

// Minimal alias map for Phase 1 (full alias_engine is Phase 2).
// Maps a normalized key -> osis. Numbered books are handled separately.
fn abbrev_to_osis(key: &str) -> Option<&'static str> {
    let m: &[(&str, &str)] = &[
        ("gen", "Gen"), ("genesis", "Gen"),
        ("exod", "Exod"), ("exodus", "Exod"), ("ex", "Exod"),
        ("lev", "Lev"), ("leviticus", "Lev"),
        ("num", "Num"), ("numbers", "Num"),
        ("deut", "Deut"), ("deuteronomy", "Deut"),
        ("josh", "Josh"), ("joshua", "Josh"),
        ("judg", "Judg"), ("judges", "Judg"),
        ("ruth", "Ruth"),
        ("ezra", "Ezra"), ("neh", "Neh"), ("nehemiah", "Neh"),
        ("esth", "Esth"), ("esther", "Esth"),
        ("job", "Job"),
        ("ps", "Ps"), ("psalm", "Ps"), ("psalms", "Ps"),
        ("prov", "Prov"), ("proverbs", "Prov"),
        ("eccl", "Eccl"), ("ecclesiastes", "Eccl"),
        ("song", "Song"), ("song of solomon", "Song"), ("songofsolomon", "Song"),
        ("isa", "Isa"), ("isaiah", "Isa"),
        ("jer", "Jer"), ("jeremiah", "Jer"),
        ("lam", "Lam"), ("lamentations", "Lam"),
        ("ezek", "Ezek"), ("ezekiel", "Ezek"),
        ("dan", "Dan"), ("daniel", "Dan"),
        ("hos", "Hos"), ("hosea", "Hos"),
        ("joel", "Joel"), ("amos", "Amos"),
        ("obad", "Obad"), ("obadiah", "Obad"),
        ("jonah", "Jonah"), ("mic", "Mic"), ("micah", "Mic"),
        ("nah", "Nah"), ("nahum", "Nah"),
        ("hab", "Hab"), ("habakkuk", "Hab"),
        ("zeph", "Zeph"), ("zephaniah", "Zeph"),
        ("hag", "Hag"), ("haggai", "Hag"),
        ("zech", "Zech"), ("zechariah", "Zech"),
        ("mal", "Mal"), ("malachi", "Mal"),
        ("matt", "Matt"), ("matthew", "Matt"), ("mt", "Matt"),
        ("mark", "Mark"), ("mk", "Mark"),
        ("luke", "Luke"), ("lk", "Luke"),
        ("john", "John"), ("jn", "John"),
        ("acts", "Acts"),
        ("rom", "Rom"), ("romans", "Rom"),
        ("gal", "Gal"), ("galatians", "Gal"),
        ("eph", "Eph"), ("ephesians", "Eph"),
        ("phil", "Phil"), ("philippians", "Phil"),
        ("col", "Col"), ("colossians", "Col"),
        ("titus", "Titus"),
        ("phlm", "Phlm"), ("philemon", "Phlm"),
        ("heb", "Heb"), ("hebrews", "Heb"),
        ("jas", "Jas"), ("james", "Jas"),
        ("jude", "Jude"),
        ("rev", "Rev"), ("revelation", "Rev"),
    ];
    m.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

// Turns leading "1/2/3", "i/ii/iii", "first/second/third" into a digit prefix.
// Returns (ordinal_digit, rest) e.g. "First Corinthians" -> (Some(1), "corinthians").
fn split_ordinal(norm: &str) -> (Option<u8>, String) {
    let words: Vec<&str> = norm.split_whitespace().collect();
    if let Some(first) = words.first() {
        let ord = match *first {
            "1" | "i" | "first" => Some(1),
            "2" | "ii" | "second" => Some(2),
            "3" | "iii" | "third" => Some(3),
            _ => None,
        };
        if ord.is_some() {
            return (ord, words[1..].join(" "));
        }
    }
    (None, norm.to_string())
}

// Stems for numbered books (used only when an ordinal prefix is present).
fn numbered_stem(key: &str) -> Option<&'static str> {
    let m: &[(&str, &str)] = &[
        ("sam", "Sam"), ("samuel", "Sam"),
        ("kgs", "Kgs"), ("kings", "Kgs"),
        ("chr", "Chr"), ("chronicles", "Chr"),
        ("cor", "Cor"), ("corinthians", "Cor"),
        ("thess", "Thess"), ("thessalonians", "Thess"),
        ("tim", "Tim"), ("timothy", "Tim"),
        ("pet", "Pet"), ("peter", "Pet"),
        ("john", "John"), ("jn", "John"),
    ];
    m.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// Look up a book directly by its OSIS id (used to render display names).
pub fn book_by_osis(osis: &str) -> Option<&'static CanonicalBook> {
    BOOKS.iter().find(|b| b.osis == osis)
}

/// The next book in canonical order (None past Revelation).
pub fn book_after(osis: &str) -> Option<&'static CanonicalBook> {
    let b = book_by_osis(osis)?;
    BOOKS.iter().find(|x| x.order == b.order + 1)
}

/// The previous book in canonical order (None before Genesis).
pub fn book_before(osis: &str) -> Option<&'static CanonicalBook> {
    let b = book_by_osis(osis)?;
    if b.order <= 1 {
        return None;
    }
    BOOKS.iter().find(|x| x.order == b.order - 1)
}

/// Exact match first, then fuzzy recovery for near-misses from speech-to-text
/// (e.g. "roman" → Romans, "mathew" → Matthew, "revelations" → Revelation).
pub fn resolve_book_fuzzy(input: &str) -> Option<&'static CanonicalBook> {
    if let Some(b) = resolve_book(input) {
        return Some(b);
    }
    let norm = input.trim().to_lowercase();
    if norm.len() < 3 {
        return None;
    }
    let mut best: Option<(&'static CanonicalBook, f64)> = None;
    for b in BOOKS {
        let score = strsim::jaro_winkler(&norm, &b.name.to_lowercase());
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((b, score));
        }
    }
    match best {
        Some((b, s)) if s >= 0.90 => Some(b),
        _ => None,
    }
}

pub fn resolve_book(input: &str) -> Option<&'static CanonicalBook> {
    let norm = input.trim().to_lowercase();
    let (ord, rest) = split_ordinal(&norm);
    let rest_key = rest.replace(' ', "");

    let target_osis: String = if let Some(n) = ord {
        // numbered book: ordinal + base stem (e.g. 1 + "Cor" -> "1Cor")
        let stem = numbered_stem(&rest).or_else(|| numbered_stem(&rest_key))?;
        format!("{n}{stem}")
    } else {
        abbrev_to_osis(&rest)
            .or_else(|| abbrev_to_osis(&rest_key))?
            .to_string()
    };
    book_by_osis(&target_osis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_full_names_case_insensitively() {
        assert_eq!(resolve_book("John").unwrap().osis, "John");
        assert_eq!(resolve_book("  john ").unwrap().osis, "John");
        assert_eq!(resolve_book("PSALMS").unwrap().osis, "Ps");
    }

    #[test]
    fn resolves_numbered_books() {
        assert_eq!(resolve_book("1 Corinthians").unwrap().osis, "1Cor");
        assert_eq!(resolve_book("First Corinthians").unwrap().osis, "1Cor");
        assert_eq!(resolve_book("1 cor").unwrap().osis, "1Cor");
        assert_eq!(resolve_book("2 John").unwrap().osis, "2John");
    }

    #[test]
    fn resolves_common_abbreviations() {
        assert_eq!(resolve_book("Gen").unwrap().osis, "Gen");
        assert_eq!(resolve_book("Rom").unwrap().osis, "Rom");
        assert_eq!(resolve_book("Ps").unwrap().osis, "Ps");
    }

    #[test]
    fn rejects_unknown() {
        assert!(resolve_book("Hogwarts").is_none());
    }
}
