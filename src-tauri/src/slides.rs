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

fn split_trailing_number(s: &str) -> (String, Option<String>) {
    let t = s.trim();
    let digits: String = t.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return (t.to_string(), None);
    }
    let digits: String = digits.chars().rev().collect();
    let word = t[..t.len() - digits.len()].trim().to_string();
    (word, Some(digits))
}

/// Recognise a lyric section header ("Verse 1", "Chorus", "[Bridge]",
/// "Pre-Chorus:") as a canonical label, or None if the line isn't purely a
/// marker. The whole line must be the marker, so real lyrics like "Chorus of
/// angels" never match.
fn parse_marker(line: &str) -> Option<String> {
    let s = line
        .trim()
        .trim_start_matches(['[', '('])
        .trim_end_matches([']', ')'])
        .trim()
        .trim_end_matches(':')
        .trim();
    if s.is_empty() {
        return None;
    }
    let (word, num) = split_trailing_number(&s.to_lowercase());
    let canonical = match word.trim() {
        "verse" | "v" => "Verse",
        "chorus" | "c" => "Chorus",
        "pre-chorus" | "prechorus" | "pre chorus" => "Pre-Chorus",
        "bridge" | "b" => "Bridge",
        "refrain" => "Refrain",
        "tag" => "Tag",
        "intro" => "Intro",
        "outro" | "ending" => "Ending",
        "interlude" => "Interlude",
        "vamp" => "Vamp",
        "coda" => "Coda",
        _ => return None,
    };
    Some(match num {
        Some(n) => format!("{canonical} {n}"),
        None => canonical.to_string(),
    })
}

/// Split a slide into its optional section label and the lyric text to show. The
/// label (if any) is stripped from the projected text so markers never reach the
/// wall — they only guide the operator. A slide that is *only* a marker keeps its
/// text (nothing to strip to).
pub fn section_label(text: &str) -> (Option<String>, String) {
    let trimmed = text.trim_start();
    let (first, rest) = match trimmed.split_once('\n') {
        Some((a, b)) => (a, b),
        None => (trimmed, ""),
    };
    match parse_marker(first) {
        Some(label) if !rest.trim().is_empty() => (Some(label), rest.trim_start().to_string()),
        _ => (None, text.to_string()),
    }
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

    #[test]
    fn section_label_detects_and_strips_markers() {
        let (label, text) = section_label("Verse 1\nAmazing grace\nhow sweet");
        assert_eq!(label, Some("Verse 1".to_string()));
        assert_eq!(text, "Amazing grace\nhow sweet");

        assert_eq!(section_label("[Chorus]\nline").0, Some("Chorus".to_string()));
        assert_eq!(section_label("Pre-Chorus:\nline").0, Some("Pre-Chorus".to_string()));
        assert_eq!(section_label("BRIDGE\nline").0, Some("Bridge".to_string()));
    }

    #[test]
    fn section_label_ignores_real_lyric_lines() {
        // A line that merely starts with a section word is not a marker.
        let (label, text) = section_label("Chorus of angels sing\nto the King");
        assert_eq!(label, None);
        assert_eq!(text, "Chorus of angels sing\nto the King");
        // A lone marker with no lyric keeps its text (nothing to strip to).
        assert_eq!(section_label("Chorus"), (None, "Chorus".to_string()));
    }
}
