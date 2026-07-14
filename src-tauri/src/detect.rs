use crate::books::{resolve_book, resolve_book_fuzzy};
use crate::reference::ParsedRef;

/// How a reference was recognized — drives confidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetectSource {
    Explicit,    // exact book name + numbers
    Fuzzy,       // book name recovered by fuzzy match
    Context,     // bare "verse N" / "chapter N" resolved via remembered book
    Descriptive, // "last book of the Old Testament", "third book of Moses"
    Story,       // famous story/passage ("the prodigal son")
}

#[derive(Debug, Clone)]
pub struct Detection {
    pub reference: ParsedRef,
    pub source: DetectSource,
    /// End verse of a spoken range ("John 3:16 through 18" → Some(18)). The
    /// start verse lives in `reference.verse`.
    pub verse_end: Option<u16>,
}

/// Remembered book/chapter so later bare mentions ("verse 28") resolve.
#[derive(Debug, Clone, Default)]
pub struct RefContext {
    pub book_osis: Option<String>,
    pub chapter: Option<u16>,
}

impl RefContext {
    pub fn clear(&mut self) {
        self.book_osis = None;
        self.chapter = None;
    }
}

/// Pull number(s) out of a token, treating ANY non-digit as a separator, so
/// whatever symbol whisper inserts between chapter and verse works:
/// "3:16", "3.16", "7-7", "7/7" all yield (chapter, Some(verse)); "7" yields
/// (7, None). This is the robust "a book is followed by numbers" heuristic.
fn parse_num_token(tok: &str) -> Option<(u16, Option<u16>)> {
    let nums: Vec<u16> = tok
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<u16>().ok())
        .collect();
    match nums.as_slice() {
        [] => None,
        [a] => Some((*a, None)),
        [a, b, ..] => Some((*a, Some(*b))),
    }
}

fn word_value(word: &str) -> Option<u16> {
    let v = match word {
        "zero" => 0, "one" => 1, "two" => 2, "three" => 3, "four" => 4,
        "five" => 5, "six" => 6, "seven" => 7, "eight" => 8, "nine" => 9,
        "ten" => 10, "eleven" => 11, "twelve" => 12, "thirteen" => 13,
        "fourteen" => 14, "fifteen" => 15, "sixteen" => 16, "seventeen" => 17,
        "eighteen" => 18, "nineteen" => 19, "twenty" => 20, "thirty" => 30,
        "forty" => 40, "fifty" => 50, "sixty" => 60, "seventy" => 70,
        "eighty" => 80, "ninety" => 90,
        _ => return None,
    };
    Some(v)
}

fn fold_number_words(tokens: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        if let Some(v) = word_value(&tokens[i]) {
            if v >= 20 && v % 10 == 0 {
                if let Some(ones) = tokens.get(i + 1).and_then(|t| word_value(t)) {
                    if (1..=9).contains(&ones) {
                        out.push((v + ones).to_string());
                        i += 2;
                        continue;
                    }
                }
            }
            out.push(v.to_string());
        } else {
            out.push(tokens[i].clone());
        }
        i += 1;
    }
    out
}

fn eq_ci(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn tokenize(text: &str) -> Vec<String> {
    let cleaned: Vec<String> = text
        .split_whitespace()
        .map(|t| {
            t.trim_matches(|c: char| !c.is_alphanumeric() && c != ':')
                .to_lowercase()
        })
        .filter(|t| !t.is_empty())
        .collect();
    fold_number_words(cleaned)
}

/// Read an optional "verse N" after a chapter has been consumed at index `j`.
/// Returns (start_verse, glued_end, next_index) — a token like "16-18" yields a
/// glued end of Some(18).
///
/// Preachers rarely say the bare form. "Matthew chapter 5 from verse 3",
/// "starting at verse 3", "beginning in verse 3" all mean the same thing, and
/// dropping the lead-in used to lose the verse entirely — the reference resolved
/// to the chapter only, even when whisper had heard every word correctly.
fn read_trailing_verse(tokens: &[String], j: usize) -> (Option<u16>, Option<u16>, usize) {
    const LEAD_INS: &[&str] = &["from", "starting", "start", "beginning", "begin", "at", "in", "on"];
    let mut k = j;
    let mut saw_lead_in = false;
    let mut saw_from = false;
    while k < tokens.len() && LEAD_INS.contains(&tokens[k].to_lowercase().as_str()) {
        saw_from |= eq_ci(&tokens[k], "from");
        saw_lead_in = true;
        k += 1;
    }
    let mut saw_verse = false;
    if k < tokens.len() && (eq_ci(&tokens[k], "verse") || eq_ci(&tokens[k], "verses")) {
        saw_verse = true;
        k += 1;
    }
    // "in"/"on"/"at" are ordinary words — "Romans 8 in the NIV", "Psalm 23 on
    // Sunday" — so a bare number after them means nothing. Only "from 3" reads as
    // a verse without the word "verse" following it.
    if saw_lead_in && !saw_verse && !saw_from {
        return (None, None, j);
    }
    // And only commit to the lead-in at all if a number actually follows, or
    // "Matthew chapter 5 from the beginning" would eat tokens and find nothing.
    match tokens.get(k).and_then(|t| parse_num_token(t)) {
        Some((v, end)) => (Some(v), end, k + 1),
        None => (None, None, j),
    }
}

/// Sentinel range end meaning "to the last verse of the chapter"; the caller
/// resolves it against the actual chapter length.
pub const TO_CHAPTER_END: u16 = u16::MAX;

/// After a verse at index `j`, read a spoken range end: a connector word
/// ("to", "through", "thru", "until", "till", "and") followed by a number or by
/// "the end" (→ end of chapter). Returns (end_verse, next_index).
fn read_range_end(tokens: &[String], j: usize) -> (Option<u16>, usize) {
    const CONNECTORS: &[&str] = &["to", "through", "thru", "until", "till", "and"];
    if j < tokens.len() && CONNECTORS.contains(&tokens[j].as_str()) {
        let mut k = j + 1;
        // "...to the end [of the chapter]"
        let mut e = k;
        if e < tokens.len() && eq_ci(&tokens[e], "the") {
            e += 1;
        }
        if e < tokens.len() && eq_ci(&tokens[e], "end") {
            return (Some(TO_CHAPTER_END), e + 1);
        }
        if k < tokens.len() && (eq_ci(&tokens[k], "verse") || eq_ci(&tokens[k], "verses")) {
            k += 1;
        }
        if let Some((n, _)) = tokens.get(k).and_then(|t| parse_num_token(t)) {
            return (Some(n), k + 1);
        }
    }
    (None, j)
}

/// When a book+chapter has no verse yet, read a chapter-span phrase:
/// "the whole chapter"/"the entire chapter" → verses 1..end; "the first N
/// verses" → verses 1..N. Returns (start_verse, end_verse, next_index).
fn read_chapter_span(tokens: &[String], j: usize) -> (Option<u16>, Option<u16>, usize) {
    let mut k = j;
    if k < tokens.len() && eq_ci(&tokens[k], "the") {
        k += 1;
    }
    if k < tokens.len() && (eq_ci(&tokens[k], "whole") || eq_ci(&tokens[k], "entire")) {
        let mut kk = k + 1;
        while kk < tokens.len()
            && matches!(tokens[kk].as_str(), "of" | "it" | "thing" | "chapter" | "passage")
        {
            kk += 1;
        }
        return (Some(1), Some(TO_CHAPTER_END), kk);
    }
    if k < tokens.len() && eq_ci(&tokens[k], "first") {
        if let Some((n, _)) = tokens.get(k + 1).and_then(|t| parse_num_token(t)) {
            let mut kk = k + 2;
            if kk < tokens.len() && (eq_ci(&tokens[kk], "verses") || eq_ci(&tokens[kk], "verse")) {
                kk += 1;
            }
            return (Some(1), Some(n), kk);
        }
    }
    (None, None, j)
}

/// Range end valid only if strictly after the start.
fn valid_end(start: Option<u16>, end: Option<u16>) -> Option<u16> {
    match (start, end) {
        (Some(s), Some(e)) if e > s => Some(e),
        _ => None,
    }
}

/// Detect references in a transcript, using and updating `ctx` so bare
/// continuations ("look at verse 28") resolve against a remembered book/chapter.
pub fn detect_with_context(text: &str, ctx: &mut RefContext) -> Vec<Detection> {
    let tokens = tokenize(text);
    let mut out: Vec<Detection> = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        // 1) Full reference: <book> [chapter] N [verse] M
        let mut book: Option<(String, usize, DetectSource)> = None;
        for len in (1..=3).rev() {
            if i + len <= tokens.len() {
                let joined = tokens[i..i + len].join(" ");
                if let Some(b) = resolve_book(&joined) {
                    book = Some((b.osis.to_string(), len, DetectSource::Explicit));
                    break;
                }
            }
        }
        // Descriptive: "last book of the Old Testament", "third book of Moses".
        if book.is_none() {
            if let Some((osis, k)) = crate::knowledge::resolve_descriptive(&tokens, i) {
                book = Some((osis.to_string(), k, DetectSource::Descriptive));
            }
        }
        if book.is_none() {
            for len in 1..=2 {
                if i + len <= tokens.len() {
                    let joined = tokens[i..i + len].join(" ");
                    if let Some(b) = resolve_book_fuzzy(&joined) {
                        book = Some((b.osis.to_string(), len, DetectSource::Fuzzy));
                        break;
                    }
                }
            }
        }

        if let Some((osis, len, source)) = book {
            let single = crate::books::is_single_chapter(&osis);
            // "the whole chapter of Psalm 23" / "all of Romans 8": the span phrase
            // precedes the book, so look back a few tokens for it.
            let leading_whole = {
                let start = i.saturating_sub(3);
                tokens[start..i].iter().any(|t| eq_ci(t, "whole") || eq_ci(t, "entire"))
                    || tokens[start..i].windows(2).any(|w| eq_ci(&w[0], "all") && eq_ci(&w[1], "of"))
            };
            let mut j = i + len;
            if j < tokens.len() && eq_ci(&tokens[j], "chapter") {
                j += 1;
            }
            // Single-chapter book cited directly by verse: "Jude verse 24" → 1:24.
            if single && j < tokens.len() && (eq_ci(&tokens[j], "verse") || eq_ci(&tokens[j], "verses")) {
                if let Some((v, glued)) = tokens.get(j + 1).and_then(|t| parse_num_token(t)) {
                    let (conn_end, nj) = read_range_end(&tokens, j + 2);
                    let verse_end = valid_end(Some(v), glued.or(conn_end));
                    ctx.book_osis = Some(osis.clone());
                    ctx.chapter = Some(1);
                    out.push(Detection {
                        reference: ParsedRef { book_osis: osis.clone(), chapter: 1, verse: Some(v) },
                        source,
                        verse_end,
                    });
                    i = nj;
                    continue;
                }
            }
            if let Some((n1, verse_in_tok)) = tokens.get(j).and_then(|t| parse_num_token(t)) {
                j += 1;
                let mut chapter = n1;
                let mut glued_end = None;
                let mut verse = if verse_in_tok.is_some() {
                    verse_in_tok
                } else {
                    let (v, end, nj) = read_trailing_verse(&tokens, j);
                    j = nj;
                    glued_end = end;
                    v
                };
                // Chapter-span phrase: "John 3 the whole chapter", "...first 3 verses".
                if verse.is_none() {
                    let (v, e, nj) = read_chapter_span(&tokens, j);
                    if v.is_some() {
                        verse = v;
                        glued_end = e;
                        j = nj;
                    } else if leading_whole {
                        verse = Some(1);
                        glued_end = Some(TO_CHAPTER_END);
                    }
                }
                // Single-chapter book with a lone number: that number is the verse.
                if single && verse.is_none() {
                    verse = Some(chapter);
                    chapter = 1;
                }
                let (conn_end, nj) = read_range_end(&tokens, j);
                j = nj;
                let verse_end = valid_end(verse, glued_end.or(conn_end));
                ctx.book_osis = Some(osis.clone());
                ctx.chapter = Some(chapter);
                out.push(Detection {
                    reference: ParsedRef { book_osis: osis.clone(), chapter, verse },
                    source,
                    verse_end,
                });
                i = j;
                continue;
            }
            // A descriptive book with no number yet — remember it so a following
            // "chapter 3" continuation resolves ("last book of the OT ... chapter 3").
            if source == DetectSource::Descriptive {
                ctx.book_osis = Some(osis);
                ctx.chapter = None;
                i += len;
                continue;
            }
        }

        // 2) Continuation "chapter N [verse M]" against remembered book.
        if eq_ci(&tokens[i], "chapter") {
            if let (Some(book_osis), Some((chapter, verse_in_tok))) = (
                ctx.book_osis.clone(),
                tokens.get(i + 1).and_then(|t| parse_num_token(t)),
            ) {
                let mut j = i + 2;
                let mut glued_end = None;
                let verse = if verse_in_tok.is_some() {
                    verse_in_tok
                } else {
                    let (v, end, nj) = read_trailing_verse(&tokens, j);
                    j = nj;
                    glued_end = end;
                    v
                };
                let (conn_end, nj) = read_range_end(&tokens, j);
                j = nj;
                let verse_end = valid_end(verse, glued_end.or(conn_end));
                ctx.chapter = Some(chapter);
                out.push(Detection {
                    reference: ParsedRef { book_osis, chapter, verse },
                    source: DetectSource::Context,
                    verse_end,
                });
                i = j;
                continue;
            }
        }

        // 3) Continuation "verse M" against remembered book + chapter.
        if eq_ci(&tokens[i], "verse") || eq_ci(&tokens[i], "verses") {
            if let (Some(book_osis), Some(chapter), Some((verse, glued))) = (
                ctx.book_osis.clone(),
                ctx.chapter,
                tokens.get(i + 1).and_then(|t| parse_num_token(t)),
            ) {
                let (conn_end, nj) = read_range_end(&tokens, i + 2);
                let verse_end = valid_end(Some(verse), glued.or(conn_end));
                out.push(Detection {
                    reference: ParsedRef { book_osis, chapter, verse: Some(verse) },
                    source: DetectSource::Context,
                    verse_end,
                });
                i = nj;
                continue;
            }
        }

        i += 1;
    }

    // Famous stories/passages ("the prodigal son" → Luke 15:11), added as
    // suggestions unless that book+chapter was already detected explicitly.
    for (osis, chapter, verse) in crate::knowledge::detect_stories(text) {
        let dup = out
            .iter()
            .any(|d| d.reference.book_osis == osis && d.reference.chapter == chapter);
        if !dup {
            out.push(Detection {
                reference: ParsedRef { book_osis: osis, chapter, verse: Some(verse) },
                source: DetectSource::Story,
                verse_end: None,
            });
        }
    }
    out
}

/// Detect a spoken relative-navigation command (against the presented verse).
pub fn detect_nav_command(text: &str) -> Option<&'static str> {
    let t = text.to_lowercase();
    if t.contains("next chapter") {
        Some("next-chapter")
    } else if t.contains("previous chapter") || t.contains("last chapter") {
        Some("prev-chapter")
    } else if t.contains("next verse") || t.contains("following verse") {
        Some("next-verse")
    } else if t.contains("previous verse") || t.contains("verse before") || t.contains("go back") {
        Some("prev-verse")
    } else {
        None
    }
}

/// Stateless convenience: detect full references with no carried context.
#[cfg_attr(not(test), allow(dead_code))] // used by unit tests
pub fn detect_references(text: &str) -> Vec<ParsedRef> {
    let mut ctx = RefContext::default();
    detect_with_context(text, &mut ctx)
        .into_iter()
        .map(|d| d.reference)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str) -> ParsedRef {
        let r = detect_references(text);
        assert_eq!(r.len(), 1, "expected 1 ref in {text:?}, got {r:?}");
        r.into_iter().next().unwrap()
    }

    #[test]
    fn digits_and_spoken_numbers() {
        let a = one("please turn to John 3:16 with me");
        assert_eq!((a.book_osis.as_str(), a.chapter, a.verse), ("John", 3, Some(16)));
        let b = one("reading from Romans chapter eight verse twenty eight");
        assert_eq!((b.book_osis.as_str(), b.chapter, b.verse), ("Rom", 8, Some(28)));
        let c = one("open your bible to John chapter three sixteen");
        assert_eq!((c.book_osis.as_str(), c.chapter, c.verse), ("John", 3, Some(16)));
        let d = one("first corinthians thirteen");
        assert_eq!((d.book_osis.as_str(), d.chapter, d.verse), ("1Cor", 13, None));
    }

    #[test]
    fn recovers_fuzzy_book_names() {
        let a = one("roman chapter 8 verse 28");
        assert_eq!((a.book_osis.as_str(), a.chapter, a.verse), ("Rom", 8, Some(28)));
        let b = one("revelations chapter 22 verse 20");
        assert_eq!(b.book_osis, "Rev");
    }

    /// Transcripts taken verbatim from real speech. Every one of these was heard
    /// well enough to act on, and every one used to resolve to nothing.
    #[test]
    fn reads_a_verse_introduced_by_a_lead_in_word() {
        for said in [
            "turn with me to matthew chapter 5 from verse 3",
            "matthew chapter 5 starting at verse 3",
            "matthew chapter 5 beginning in verse 3",
            "matthew chapter 5 at verse 3",
            "matthew chapter 5 from 3",
        ] {
            let d = one(said);
            assert_eq!(
                (d.book_osis.as_str(), d.chapter, d.verse),
                ("Matt", 5, Some(3)),
                "failed on: {said}"
            );
        }
    }

    /// "in"/"on"/"at" are ordinary words. A number after them is not a verse
    /// unless the speaker actually said "verse".
    #[test]
    fn ordinary_words_after_a_chapter_do_not_invent_a_verse() {
        let a = one("romans 8 in the niv");
        assert_eq!((a.chapter, a.verse), (8, None));
        let b = one("let's read psalm 23 on sunday");
        assert_eq!((b.chapter, b.verse), (23, None));
    }

    /// Whisper drops the leading consonant of Nehemiah constantly. Fuzzy matching
    /// scores these closest to Zephaniah and Nahum, so they are listed explicitly.
    #[test]
    fn recovers_nehemiah_from_real_mishearings() {
        for said in [
            "hemaiah chapter 8 verse 10",
            "nahimiah chapter 8 verse 10",
            "nehimiah chapter 8 verse 10",
        ] {
            let d = one(said);
            assert_eq!(
                (d.book_osis.as_str(), d.chapter, d.verse),
                ("Neh", 8, Some(10)),
                "failed on: {said}"
            );
        }
    }

    #[test]
    fn context_carryover_across_utterances() {
        let mut ctx = RefContext::default();
        let first = detect_with_context("turn with me to Romans chapter 8", &mut ctx);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].reference.chapter, 8);

        // later, no book spoken:
        let later = detect_with_context("now look at verse 28", &mut ctx);
        assert_eq!(later.len(), 1);
        let r = &later[0].reference;
        assert_eq!((r.book_osis.as_str(), r.chapter, r.verse), ("Rom", 8, Some(28)));
        assert_eq!(later[0].source, DetectSource::Context);
    }

    #[test]
    fn handles_dot_and_implied_forms() {
        // whisper renders "John two five" as "John 2.5" (dot, no "chapter/verse")
        let a = one("let's turn our bibles to John 2.5 in the ASV translation");
        assert_eq!((a.book_osis.as_str(), a.chapter, a.verse), ("John", 2, Some(5)));

        // implied "John 2 5" (two bare numbers, no "chapter/verse" words)
        let b = one("turn to John 2 5");
        assert_eq!((b.book_osis.as_str(), b.chapter, b.verse), ("John", 2, Some(5)));

        // hyphen glue: "Matthew 7-7" (spoken "Matthew seven seven")
        let c = one("let's open our bibles to Matthew 7-7 King James Version");
        assert_eq!((c.book_osis.as_str(), c.chapter, c.verse), ("Matt", 7, Some(7)));

        // slash glue
        let d = one("turn to Romans 8/28");
        assert_eq!((d.book_osis.as_str(), d.chapter, d.verse), ("Rom", 8, Some(28)));
    }

    #[test]
    fn single_chapter_books_cited_by_verse() {
        // "Jude 24" means Jude 1:24, not chapter 24
        let a = one("let's read Jude 24");
        assert_eq!((a.book_osis.as_str(), a.chapter, a.verse), ("Jude", 1, Some(24)));

        // explicit "Jude verse 24"
        let b = one("turn to Jude verse 24");
        assert_eq!((b.book_osis.as_str(), b.chapter, b.verse), ("Jude", 1, Some(24)));

        // "Philemon 6" → Philemon 1:6
        let c = one("open to Philemon 6");
        assert_eq!((c.book_osis.as_str(), c.chapter, c.verse), ("Phlm", 1, Some(6)));

        // explicit chapter still respected: "Jude 1:24"
        let d = one("Jude 1:24");
        assert_eq!((d.book_osis.as_str(), d.chapter, d.verse), ("Jude", 1, Some(24)));

        // a normal multi-chapter book is unaffected: "Romans 8" stays chapter 8
        let e = one("Romans 8");
        assert_eq!((e.book_osis.as_str(), e.chapter, e.verse), ("Rom", 8, None));
    }

    #[test]
    fn descriptive_book_names() {
        // "last book of the Old Testament" = Malachi
        let a = one("turn to the last book of the old testament chapter 3");
        assert_eq!((a.book_osis.as_str(), a.chapter, a.verse), ("Mal", 3, None));

        // "third book of Moses" = Leviticus
        let b = one("the third book of moses chapter 1 verse 1");
        assert_eq!((b.book_osis.as_str(), b.chapter, b.verse), ("Lev", 1, Some(1)));

        // "last book of the New Testament" = Revelation
        let c = one("the last book of the new testament 22 20");
        assert_eq!((c.book_osis.as_str(), c.chapter, c.verse), ("Rev", 22, Some(20)));
    }

    #[test]
    fn famous_stories_resolve() {
        let a = one("let me tell you about the prodigal son this morning");
        assert_eq!((a.book_osis.as_str(), a.chapter, a.verse), ("Luke", 15, Some(11)));

        let b = one("remember david and goliath");
        assert_eq!((b.book_osis.as_str(), b.chapter, b.verse), ("1Sam", 17, Some(1)));

        let mut ctx = RefContext::default();
        let d = detect_with_context("the story of noah's ark", &mut ctx);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].source, DetectSource::Story);
        assert_eq!(d[0].reference.book_osis, "Gen");
    }

    #[test]
    fn loose_paraphrase_of_a_story_resolves() {
        // No curated phrase said — only the distinctive keywords of the story.
        let a = one("there is this man who had a younger son who asked for his inheritance and wasted it feeding pigs during a famine");
        assert_eq!((a.book_osis.as_str(), a.chapter, a.verse), ("Luke", 15, Some(11)));

        // David & Goliath retold without naming it.
        let b = one("a young shepherd faced a philistine giant with only a sling");
        assert_eq!((b.book_osis.as_str(), b.chapter, b.verse), ("1Sam", 17, Some(1)));

        // Too few keywords must NOT fire (guards live projection).
        assert!(detect_references("he told a story about a younger brother").is_empty());
    }

    #[test]
    fn character_epithets_resolve_to_book() {
        let a = one("as the weeping prophet chapter 29 verse 11 says");
        assert_eq!((a.book_osis.as_str(), a.chapter, a.verse), ("Jer", 29, Some(11)));

        let b = one("the beloved physician chapter 15");
        assert_eq!((b.book_osis.as_str(), b.chapter), ("Luke", 15));
    }

    #[test]
    fn long_epithets_and_groupings_resolve() {
        // 7-token epithet must resolve (window was widened past 6).
        let a = one("turn to the shortest book of the old testament verse 15");
        assert_eq!((a.book_osis.as_str(), a.chapter, a.verse), ("Obad", 1, Some(15)));

        let b = one("the runaway prophet chapter 2");
        assert_eq!((b.book_osis.as_str(), b.chapter), ("Jonah", 2));

        // grouping → first book of the group
        let c = one("open the minor prophets chapter 1");
        assert_eq!((c.book_osis.as_str(), c.chapter), ("Hos", 1));
    }

    #[test]
    fn character_fun_facts_open_the_story() {
        let a = one("the oldest man in the bible");
        assert_eq!((a.book_osis.as_str(), a.chapter), ("Gen", 5));

        let b = one("who was the strongest man in the bible");
        assert_eq!((b.book_osis.as_str(), b.chapter), ("Judg", 16));

        let c = one("the shortest verse in the bible is only two words");
        assert_eq!((c.book_osis.as_str(), c.chapter, c.verse), ("John", 11, Some(35)));

        let d = one("the wisest man who ever lived asked god for understanding");
        assert_eq!((d.book_osis.as_str(), d.chapter), ("1Kgs", 3));
    }

    #[test]
    fn more_loose_paraphrases_resolve() {
        // Zacchaeus, unnamed
        let a = one("a short man climbed a sycamore tree to see jesus");
        assert_eq!((a.book_osis.as_str(), a.chapter), ("Luke", 19));

        // Fiery furnace, unnamed
        let b = one("three men were thrown into the furnace but a fourth appeared");
        assert_eq!((b.book_osis.as_str(), b.chapter), ("Dan", 3));

        // Rich young ruler, unnamed
        let c = one("it is harder for a camel to pass through the eye of a needle");
        assert_eq!((c.book_osis.as_str(), c.chapter), ("Matt", 19));

        // Lost sheep, described not named
        let d = one("the shepherd went out to find his one lost sheep");
        assert_eq!((d.book_osis.as_str(), d.chapter), ("Luke", 15));

        // Re-added flagship stories still resolve
        let e = one("he rebuked the wind and the waves and there was a great calm");
        assert_eq!((e.book_osis.as_str(), e.chapter), ("Matt", 8));
        let f = one("she touched the hem of his garment and was made whole");
        assert_eq!((f.book_osis.as_str(), f.chapter), ("Matt", 9));
        let g = one("the man at the pool of bethesda for thirty eight years");
        assert_eq!((g.book_osis.as_str(), g.chapter), ("John", 5));
    }

    #[test]
    fn spoken_verse_ranges() {
        fn one_det(text: &str) -> Detection {
            let mut ctx = RefContext::default();
            let d = detect_with_context(text, &mut ctx);
            assert_eq!(d.len(), 1, "expected 1 detection in {text:?}, got {d:?}");
            d.into_iter().next().unwrap()
        }

        // connector "through"
        let a = one_det("turn to John 3:16 through 18");
        assert_eq!((a.reference.chapter, a.reference.verse, a.verse_end), (3, Some(16), Some(18)));

        // connector "to" with explicit "verse"
        let b = one_det("Romans chapter 8 verse 28 to 30");
        assert_eq!((b.reference.chapter, b.reference.verse, b.verse_end), (8, Some(28), Some(30)));

        // glued dash token
        let c = one_det("Matthew 5 verse 3-10");
        assert_eq!((c.reference.chapter, c.reference.verse, c.verse_end), (5, Some(3), Some(10)));

        // "and" joining adjacent verses
        let d = one_det("John 3 verse 16 and 17");
        assert_eq!((d.reference.verse, d.verse_end), (Some(16), Some(17)));

        // a non-range single verse leaves verse_end None
        let e = one_det("John 3:16");
        assert_eq!(e.verse_end, None);

        // backwards / equal end is rejected
        let f = one_det("John 3:16 to 16");
        assert_eq!(f.verse_end, None);

        // "to the end" → open-ended sentinel
        let g = one_det("read John 3 verse 16 to the end");
        assert_eq!((g.reference.verse, g.verse_end), (Some(16), Some(TO_CHAPTER_END)));
    }

    #[test]
    fn spoken_chapter_spans() {
        fn one_det(text: &str) -> Detection {
            let mut ctx = RefContext::default();
            let d = detect_with_context(text, &mut ctx);
            assert_eq!(d.len(), 1, "expected 1 detection in {text:?}, got {d:?}");
            d.into_iter().next().unwrap()
        }

        // whole chapter → 1..end
        let a = one_det("let's read the whole chapter of Psalm 23");
        assert_eq!((a.reference.chapter, a.reference.verse, a.verse_end), (23, Some(1), Some(TO_CHAPTER_END)));

        // "Romans 8 the whole chapter"
        let b = one_det("Romans 8 the whole chapter");
        assert_eq!((b.reference.verse, b.verse_end), (Some(1), Some(TO_CHAPTER_END)));

        // first N verses
        let c = one_det("John chapter 1 the first 3 verses");
        assert_eq!((c.reference.chapter, c.reference.verse, c.verse_end), (1, Some(1), Some(3)));
    }

    #[test]
    fn misheard_book_names_resolve_with_a_number() {
        // Base model roughness: misheard book + number still lands the reference.
        let a = one("turn to the economy chapter 8 verse 6");
        assert_eq!((a.book_osis.as_str(), a.chapter, a.verse), ("Deut", 8, Some(6)));
        let b = one("open to philippines 4 13");
        assert_eq!((b.book_osis.as_str(), b.chapter, b.verse), ("Phil", 4, Some(13)));
        let c = one("read malikai 3 10");
        assert_eq!((c.book_osis.as_str(), c.chapter, c.verse), ("Mal", 3, Some(10)));

        // Safety: a misheard book with NO number projects nothing.
        assert!(detect_references("the economy is really struggling this year").is_empty());
    }

    #[test]
    fn ignores_non_references() {
        assert!(detect_references("and so the lord spoke to the people").is_empty());
    }

    /// Ordinary preaching/prayer speech must not trigger any scripture — guards
    /// the live projection screen against embarrassing misfires.
    #[test]
    fn no_false_positives_on_ordinary_speech() {
        let corpus = [
            "and so the lord spoke to the people that morning",
            "we thank god for his goodness and his tender mercy",
            "let us pray together as we come to a close",
            "the church must rise up and take its rightful place",
            "i want to talk to you today about faith and obedience",
            "many of us are going through difficult seasons right now",
            "god has a wonderful plan for your life and your family",
            "the enemy wants to steal your joy but he cannot have it",
            "when you give your life to christ everything begins to change",
            "we are called to love one another the way he loved us",
            "there is power in the mighty name of jesus tonight",
            "worship him with all your heart and all your soul",
            "the holy spirit is moving in this place this evening",
            "i believe god is about to do something great in you",
            "our god is a faithful god and he never ever fails",
            "let everybody say amen and give him the highest praise",
            "you have to trust the process and keep on believing",
            "some of you came in here today with a very heavy heart",
            "please remember to bring a friend along next sunday",
            "the ushers will now come and receive the morning offering",
            "a young man came to me after service asking for prayer",
            "the storm may be raging but keep your eyes on him",
            "god is not a man that he should lie to any of us",
            "stand to your feet and lift your hands to the king",
            "he is the same yesterday today and forevermore",
            "we declare healing over every sickness and disease",
            "the anointing breaks the yoke of bondage in your life",
            "walk in love and let your light shine before all men",
            "father we surrender everything to you this hour",
            "keep believing for that breakthrough it is on the way",
            // Adversarial near-misses: share 1-2 keywords, must still stay silent.
            "the widow came forward and gave her offering cheerfully",
            "he was a rich young man with a bright future ahead",
            "there was a great storm out on the open sea that night",
            "he prayed all night long alone in the quiet garden",
            "the king sat on his throne in all his glory",
            "she poured out the oil and blessed the whole house",
            "the water was deep and the fish were plenty that day",
        ];
        let mut fired: Vec<(String, String, u16, u16)> = Vec::new();
        for s in corpus {
            for d in detect_references(s) {
                fired.push((
                    s.to_string(),
                    d.book_osis.clone(),
                    d.chapter,
                    d.verse.unwrap_or(0),
                ));
            }
        }
        assert!(fired.is_empty(), "unexpected detections on ordinary speech: {fired:#?}");
    }
}
