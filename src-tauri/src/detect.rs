use crate::books::{resolve_book, resolve_book_fuzzy};
use crate::reference::ParsedRef;

/// How a reference was recognized — drives confidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetectSource {
    Explicit, // exact book name + numbers
    Fuzzy,    // book name recovered by fuzzy match
    Context,  // bare "verse N" / "chapter N" resolved via remembered book
}

#[derive(Debug, Clone)]
pub struct Detection {
    pub reference: ParsedRef,
    pub source: DetectSource,
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
fn read_trailing_verse(tokens: &[String], mut j: usize) -> (Option<u16>, usize) {
    if j < tokens.len() && (eq_ci(&tokens[j], "verse") || eq_ci(&tokens[j], "verses")) {
        j += 1;
    }
    if let Some((v, _)) = tokens.get(j).and_then(|t| parse_num_token(t)) {
        (Some(v), j + 1)
    } else {
        (None, j)
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
            let mut j = i + len;
            if j < tokens.len() && eq_ci(&tokens[j], "chapter") {
                j += 1;
            }
            if let Some((chapter, verse_in_tok)) = tokens.get(j).and_then(|t| parse_num_token(t)) {
                j += 1;
                let verse = if verse_in_tok.is_some() {
                    verse_in_tok
                } else {
                    let (v, nj) = read_trailing_verse(&tokens, j);
                    j = nj;
                    v
                };
                ctx.book_osis = Some(osis.clone());
                ctx.chapter = Some(chapter);
                out.push(Detection {
                    reference: ParsedRef { book_osis: osis, chapter, verse },
                    source,
                });
                i = j;
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
                let verse = if verse_in_tok.is_some() {
                    verse_in_tok
                } else {
                    let (v, nj) = read_trailing_verse(&tokens, j);
                    j = nj;
                    v
                };
                ctx.chapter = Some(chapter);
                out.push(Detection {
                    reference: ParsedRef { book_osis, chapter, verse },
                    source: DetectSource::Context,
                });
                i = j;
                continue;
            }
        }

        // 3) Continuation "verse M" against remembered book + chapter.
        if eq_ci(&tokens[i], "verse") || eq_ci(&tokens[i], "verses") {
            if let (Some(book_osis), Some(chapter), Some((verse, _))) = (
                ctx.book_osis.clone(),
                ctx.chapter,
                tokens.get(i + 1).and_then(|t| parse_num_token(t)),
            ) {
                out.push(Detection {
                    reference: ParsedRef { book_osis, chapter, verse: Some(verse) },
                    source: DetectSource::Context,
                });
                i += 2;
                continue;
            }
        }

        i += 1;
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
    fn ignores_non_references() {
        assert!(detect_references("and so the lord spoke to the people").is_empty());
    }
}
