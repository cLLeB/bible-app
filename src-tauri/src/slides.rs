/// Split raw song lyrics into slides. A blank line (one or more) separates
/// slides; surrounding whitespace is trimmed and empty blocks are dropped.
pub fn split_lyrics(lyrics: &str) -> Vec<String> {
    lyrics
        .replace("\r\n", "\n")
        .split("\n\n")
        .map(|block| block.trim())
        .filter(|block| !block.is_empty())
        .map(|block| block.to_string())
        .collect()
}

/// Split a long passage into readable slides, each at most `max_chars`,
/// breaking only at word boundaries. A short passage stays a single slide.
pub fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    let trimmed = text.trim();
    if max_chars == 0 || trimmed.chars().count() <= max_chars {
        return vec![trimmed.to_string()];
    }
    let mut chunks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in trimmed.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > max_chars {
            chunks.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_keeps_short_text_single_and_splits_long() {
        assert_eq!(chunk_text("short verse", 220).len(), 1);
        let long = "word ".repeat(100);
        let chunks = chunk_text(long.trim(), 50);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.chars().count() <= 50));
        // no words lost
        assert_eq!(chunks.join(" ").split_whitespace().count(), 100);
    }

    #[test]
    fn splits_on_blank_lines() {
        let slides = split_lyrics("Line A1\nLine A2\n\nLine B1");
        assert_eq!(slides.len(), 2);
        assert_eq!(slides[0], "Line A1\nLine A2");
        assert_eq!(slides[1], "Line B1");
    }

    #[test]
    fn trims_and_drops_empty_blocks() {
        let slides = split_lyrics("\n\nVerse\n\n\n\nChorus\n\n");
        assert_eq!(slides, vec!["Verse".to_string(), "Chorus".to_string()]);
    }

    #[test]
    fn single_block_is_one_slide() {
        assert_eq!(split_lyrics("just one slide"), vec!["just one slide".to_string()]);
    }
}
