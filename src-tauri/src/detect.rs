use crate::books::resolve_book;
use crate::reference::ParsedRef;

/// Parse a number token like "3", "16", or "3:16" → (chapter-or-number, optional verse).
fn parse_num_token(tok: &str) -> Option<(u16, Option<u16>)> {
    if tok.contains(':') {
        let mut parts = tok.split(':');
        let a = parts.next()?.parse::<u16>().ok()?;
        let b = parts.next().and_then(|p| p.parse::<u16>().ok());
        Some((a, b))
    } else {
        tok.parse::<u16>().ok().map(|n| (n, None))
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

/// Convert spoken number words into digit tokens so "chapter three sixteen"
/// becomes "chapter 3 16" and "twenty eight" becomes "28".
fn fold_number_words(tokens: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        if let Some(v) = word_value(&tokens[i]) {
            // tens (20,30,…,90) + ones (1..9) → combined, e.g. "twenty eight" = 28
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

/// Scan free-form transcript text for Bible references. Handles digit forms
/// ("John 3:16", "1 Corinthians 13"), spoken forms ("Romans chapter 8 verse
/// 28"), and spelled-out numbers ("John chapter three sixteen").
pub fn detect_references(text: &str) -> Vec<ParsedRef> {
    let cleaned: Vec<String> = text
        .split_whitespace()
        .map(|t| {
            t.trim_matches(|c: char| !c.is_alphanumeric() && c != ':')
                .to_lowercase()
        })
        .filter(|t| !t.is_empty())
        .collect();
    let tokens = fold_number_words(cleaned);

    let mut out: Vec<ParsedRef> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        // Try to match a book name spanning 3, 2, then 1 tokens.
        let mut book: Option<(String, usize)> = None;
        for len in (1..=3).rev() {
            if i + len <= tokens.len() {
                let joined = tokens[i..i + len].join(" ");
                if let Some(b) = resolve_book(&joined) {
                    book = Some((b.osis.to_string(), len));
                    break;
                }
            }
        }

        if let Some((osis, len)) = book {
            let mut j = i + len;
            if j < tokens.len() && eq_ci(&tokens[j], "chapter") {
                j += 1;
            }
            if let Some((chapter, verse_in_tok)) = tokens.get(j).and_then(|t| parse_num_token(t)) {
                j += 1;
                let mut verse = verse_in_tok;
                if verse.is_none() {
                    if j < tokens.len() && (eq_ci(&tokens[j], "verse") || eq_ci(&tokens[j], "verses")) {
                        j += 1;
                    }
                    if let Some((v, _)) = tokens.get(j).and_then(|t| parse_num_token(t)) {
                        verse = Some(v);
                        j += 1;
                    }
                }
                out.push(ParsedRef { book_osis: osis, chapter, verse });
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
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
    fn ignores_non_references() {
        assert!(detect_references("and so the lord spoke to the people").is_empty());
    }
}
