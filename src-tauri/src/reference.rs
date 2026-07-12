use crate::books::resolve_book;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ParsedRef {
    pub book_osis: String,
    pub chapter: u16,
    pub verse: Option<u16>,
}

pub fn parse_reference(input: &str) -> Option<ParsedRef> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();

    // Walk from the end collecting number-ish tokens (may contain ':').
    let mut tail: Vec<&str> = Vec::new();
    let mut split_at = tokens.len();
    for (i, tok) in tokens.iter().enumerate().rev() {
        if tok.chars().all(|c| c.is_ascii_digit() || c == ':' || c == '.')
            && tok.chars().any(|c| c.is_ascii_digit())
        {
            tail.insert(0, tok);
            split_at = i;
        } else {
            break;
        }
    }
    if tail.is_empty() || split_at == 0 {
        return None; // no numbers, or no book portion
    }

    let book_part = tokens[..split_at].join(" ");
    let book = resolve_book(&book_part)?;

    // Flatten tail into numbers. Accept "3:16", "3", "16" tokens or "3 16".
    let mut nums: Vec<u16> = Vec::new();
    for tok in tail {
        for piece in tok.split(|c| c == ':' || c == '.') {
            if piece.is_empty() {
                continue;
            }
            nums.push(piece.parse::<u16>().ok()?);
        }
    }
    let chapter = *nums.first()?;
    let verse = nums.get(1).copied();

    Some(ParsedRef {
        book_osis: book.osis.to_string(),
        chapter,
        verse,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_book_chapter_verse() {
        let r = parse_reference("John 3:16").unwrap();
        assert_eq!(r.book_osis, "John");
        assert_eq!(r.chapter, 3);
        assert_eq!(r.verse, Some(16));
    }

    #[test]
    fn parses_chapter_only() {
        let r = parse_reference("Romans 8").unwrap();
        assert_eq!(r.book_osis, "Rom");
        assert_eq!(r.chapter, 8);
        assert_eq!(r.verse, None);
    }

    #[test]
    fn parses_numbered_book() {
        let r = parse_reference("1 Cor 13:4").unwrap();
        assert_eq!(r.book_osis, "1Cor");
        assert_eq!(r.chapter, 13);
        assert_eq!(r.verse, Some(4));
    }

    #[test]
    fn parses_space_separator() {
        let r = parse_reference("John 3 16").unwrap();
        assert_eq!(r.verse, Some(16));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_reference("hello world").is_none());
        assert!(parse_reference("John").is_none());
    }
}
