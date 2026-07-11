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

#[cfg(test)]
mod tests {
    use super::*;

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
