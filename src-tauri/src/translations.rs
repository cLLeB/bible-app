//! On-demand translation downloads from bolls.life.
//!
//! The app ships offline-first with a small bundled set; a user can add more
//! translations from the in-app catalog. Downloading needs internet once — the
//! text is then stored in the local SQLite and used fully offline forever after.
//!
//! LICENSING: the catalog lists ONLY public-domain / freely-licensed
//! translations. Copyrighted modern translations (NIV, NLT, ESV, NKJV, …) are
//! deliberately excluded — offering them would be contributory infringement even
//! though the user initiates the download.

use serde::Serialize;

/// Free / public-domain translations — safe to bundle and distribute.
/// Order = how they appear in the catalog (modern & popular first).
pub const PUBLIC_DOMAIN: &[(&str, &str)] = &[
    ("BSB", "Berean Standard Bible"),
    ("WEB", "World English Bible"),
    ("KJV", "King James Version (1769)"),
    ("ASV", "American Standard Version (1901)"),
    ("YLT", "Young's Literal Translation (1898)"),
    ("DARBY", "Darby Translation (1890)"),
    ("BBE", "Bible in Basic English (1949)"),
    ("GNV", "Geneva Bible (1599)"),
    ("DRB", "Douay-Rheims Bible"),
    ("WBT", "Webster's Bible (1833)"),
    ("LXXE", "Brenton Septuagint (English, 1851)"),
    ("LSV", "Literal Standard Version"),
];

/// Copyrighted translations — available ONLY in the Personal-tier build, for the
/// user's own private use (never distributed). bolls.life is the medium; we do
/// not redistribute these. (bolls codes.)
pub const LICENSED: &[(&str, &str)] = &[
    ("NIV", "New International Version"),
    ("NLT", "New Living Translation"),
    ("ESV", "English Standard Version"),
    ("NKJV", "New King James Version"),
    ("NASB", "New American Standard Bible"),
    ("CSB17", "Christian Standard Bible"),
    ("AMP", "Amplified Bible"),
    ("MSG", "The Message"),
    ("NET", "New English Translation"),
    ("GNT", "Good News Bible"),
    ("GNTD", "Good News Translation"),
    ("RSV", "Revised Standard Version"),
    ("NRSVCE", "New Revised Standard Version"),
    ("CEB", "Common English Bible"),
    ("CEVD", "Contemporary English Version"),
    ("CJB", "Complete Jewish Bible"),
    ("TLV", "Tree of Life Version"),
    ("LSB", "Legacy Standard Bible"),
    ("MEV", "Modern English Version"),
    ("ISV", "International Standard Version"),
    ("ERV", "Easy-to-Read Version"),
    ("NLV", "New Life Version"),
    ("NABRE", "New American Bible"),
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub code: String,
    pub name: String,
    pub installed: bool,
    pub licensed: bool,
}

/// The download catalog with each entry's installed state. Copyrighted
/// translations are included only when `include_licensed` (Personal tier).
pub fn catalog(include_licensed: bool, installed_codes: &[String]) -> Vec<CatalogEntry> {
    let installed = |code: &str| installed_codes.iter().any(|c| c == code);
    let mut out: Vec<CatalogEntry> = PUBLIC_DOMAIN
        .iter()
        .map(|(code, name)| CatalogEntry {
            code: (*code).to_string(),
            name: (*name).to_string(),
            installed: installed(code),
            licensed: false,
        })
        .collect();
    if include_licensed {
        out.extend(LICENSED.iter().map(|(code, name)| CatalogEntry {
            code: (*code).to_string(),
            name: (*name).to_string(),
            installed: installed(code),
            licensed: true,
        }));
    }
    out
}

/// Resolve a downloadable code to its display name. Copyrighted codes resolve
/// only when `include_licensed` (Personal tier); otherwise None (refused).
pub fn catalog_name(code: &str, include_licensed: bool) -> Option<&'static str> {
    if let Some((_, n)) = PUBLIC_DOMAIN.iter().find(|(c, _)| *c == code) {
        return Some(n);
    }
    if include_licensed {
        if let Some((_, n)) = LICENSED.iter().find(|(c, _)| *c == code) {
            return Some(n);
        }
    }
    None
}

/// Drop all `<...>` tags, keeping their content.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Split on `<br>` / `<br/>` / `<br />` (case-insensitive).
fn split_br(s: &str) -> Vec<&str> {
    let lower = s.to_ascii_lowercase();
    let mut segments = Vec::new();
    let mut start = 0;
    let mut search = 0;
    while let Some(rel) = lower[search..].find("<br") {
        let a = search + rel;
        // Confirm it's a <br…> tag and find its closing '>'.
        if let Some(gt) = s[a..].find('>') {
            let after = s[a + 3..].chars().next();
            if matches!(after, Some('>') | Some('/') | Some(' ')) {
                segments.push(&s[start..a]);
                start = a + gt + 1;
                search = start;
                continue;
            }
        }
        search = a + 3;
    }
    segments.push(&s[start..]);
    segments
}

fn seg_word_count(seg: &str) -> usize {
    strip_tags(seg).split_whitespace().count()
}

/// A leading `<br/>`-separated segment that is an editorial section heading
/// ("The Beginning", "The Sermon on the Mount") rather than verse text.
///
/// Deliberately strict — losing a real verse is far worse than leaving a
/// heading. A heading is short (≤6 words), title-like (≥2 capitalized words or a
/// bare number like "Psalm 23"), and carries NO terminal punctuation. Real
/// verses almost always end in `. , ; : ! ?` so exclamations ("…O LORD!") and
/// mid-clause name lists are protected.
fn is_heading(seg: &str) -> bool {
    let s = strip_tags(seg);
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    let words: Vec<&str> = s.split_whitespace().collect();
    if !(1..=6).contains(&words.len()) {
        return false;
    }
    if matches!(
        s.chars().last(),
        Some('.') | Some(',') | Some(';') | Some(':') | Some('!') | Some('?') | Some('—') | Some('–')
    ) {
        return false;
    }
    let caps = words
        .iter()
        .filter(|w| w.chars().next().is_some_and(|c| c.is_uppercase()))
        .count();
    let has_number = words.iter().any(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_digit()));
    caps >= 2 || has_number
}

/// Strip MyBible/HTML markup: drop Strong's numbers, footnotes, notes and
/// leading editorial section headings; keep the words inside formatting tags and
/// genuine poetry line breaks; normalize whitespace.
fn clean(text: &str) -> String {
    // 1) Remove <S>..</S> (Strong's), <f>..</f>, <n>..</n>, <h>..</h> with content.
    let mut s = text.to_string();
    for tag in ["S", "f", "n", "h"] {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        while let Some(a) = s.find(&open) {
            match s[a + open.len()..].find(&close) {
                Some(rel) => {
                    let end = a + open.len() + rel + close.len();
                    s.replace_range(a..end, "");
                }
                None => break, // unbalanced — leave it for the tag stripper
            }
        }
    }
    // 2) Drop leading section headings; poetry <br/> becomes a space. Trim empty
    //    leading/trailing segments (a stray <br/>) first, and only strip a
    //    heading when followed by a substantial (≥4-word) segment.
    let mut segs = split_br(&s);
    while segs.first().is_some_and(|x| strip_tags(x).trim().is_empty()) {
        segs.remove(0);
    }
    while segs.last().is_some_and(|x| strip_tags(x).trim().is_empty()) {
        segs.pop();
    }
    let mut i = 0;
    while i + 1 < segs.len() && is_heading(segs[i]) && seg_word_count(segs[i + 1]) >= 4 {
        i += 1;
    }
    let joined = segs[i..].join(" ");
    // 3) Drop any remaining tags (<J>, <i>, <e>, <pb/> …), keeping their content.
    strip_tags(&joined).split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(serde::Deserialize)]
struct BollsVerse {
    book: u8,
    chapter: u16,
    verse: u16,
    text: String,
}

/// Download a translation from bolls and convert it to our canonical seed JSON
/// (`{translation:{code,name}, verses:[{book_osis,chapter,verse,text}]}`).
/// Only canonical books 1..=66 are kept; markup is stripped.
pub fn fetch_canonical(code: &str, include_licensed: bool) -> Result<String, String> {
    let name = catalog_name(code, include_licensed).ok_or_else(|| {
        format!("'{code}' is not available in this build (copyrighted translations are Personal-tier only)")
    })?;
    let url = format!("https://bolls.life/static/translations/{code}.json");
    let rows: Vec<BollsVerse> = ureq::get(&url)
        .call()
        .map_err(|e| format!("download failed: {e}"))?
        .into_json()
        .map_err(|e| format!("parse failed: {e}"))?;

    let verses: Vec<serde_json::Value> = rows
        .into_iter()
        .filter_map(|r| {
            let osis = crate::books::osis_by_order(r.book)?;
            let text = clean(&r.text);
            if text.is_empty() {
                return None;
            }
            Some(serde_json::json!({
                "book_osis": osis,
                "chapter": r.chapter,
                "verse": r.verse,
                "text": text,
            }))
        })
        .collect();

    if verses.is_empty() {
        return Err(format!("no canonical verses found for '{code}'"));
    }
    let doc = serde_json::json!({
        "translation": { "code": code, "name": name },
        "verses": verses,
    });
    Ok(doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleaner_strips_strongs_and_tags_keeps_words() {
        assert_eq!(
            clean("In the <S>7225</S>beginning <J>God</J> <i>created</i><pb/> the heaven<f>a note</f>"),
            "In the beginning God created the heaven"
        );
        assert_eq!(clean("plain text  with   spaces"), "plain text with spaces");
    }

    #[test]
    fn cleaner_drops_section_headings_keeps_poetry() {
        // editorial section heading removed (followed by a real verse)
        assert_eq!(
            clean("The Beginning<br/>In the beginning God created the heavens and the earth."),
            "In the beginning God created the heavens and the earth."
        );
        assert_eq!(
            clean("Jesus Feeds the Five Thousand<br/>When Jesus heard what had happened, he withdrew by boat."),
            "When Jesus heard what had happened, he withdrew by boat."
        );
        // a title with a number ("Psalm 23") is removed; the ascription that ends
        // in a period is conservatively kept (never risk eating a verse)
        assert_eq!(
            clean("Psalm 23<br/>A psalm of David.<br/>The Lord is my shepherd, I shall not be in want."),
            "A psalm of David. The Lord is my shepherd, I shall not be in want."
        );
        // genuine poetry line breaks preserved (both lines kept)
        assert_eq!(
            clean("He makes me lie down in green pastures,<br/>he leads me beside quiet waters,"),
            "He makes me lie down in green pastures, he leads me beside quiet waters,"
        );
    }

    #[test]
    fn cleaner_never_eats_real_verses() {
        // trailing <br> must not turn a real verse into a "heading" (regression:
        // this previously deleted Genesis 49:18 and the Numbers census lists)
        assert_eq!(
            clean("I trust in you for salvation, O LORD!<br>"),
            "I trust in you for salvation, O LORD!"
        );
        assert_eq!(
            clean("Simeon — Shelumiel son of Zurishaddai<br>"),
            "Simeon — Shelumiel son of Zurishaddai"
        );
        // a short proper-noun-heavy verse is never mistaken for a heading
        assert_eq!(
            clean("Ephraim son of Joseph, Elishama son of Ammihud<br/>Manasseh son of Joseph, Gamaliel"),
            "Ephraim son of Joseph, Elishama son of Ammihud Manasseh son of Joseph, Gamaliel"
        );
    }

    #[test]
    fn distribution_tier_excludes_copyrighted() {
        // Distribution (include_licensed = false)
        assert!(catalog_name("NIV", false).is_none());
        assert!(catalog_name("NLT", false).is_none());
        assert!(catalog_name("BSB", false).is_some());
        assert!(catalog_name("KJV", false).is_some());
        assert!(catalog(false, &[]).iter().all(|e| !e.licensed));
    }

    #[test]
    fn personal_tier_includes_copyrighted() {
        assert!(catalog_name("NIV", true).is_some());
        assert!(catalog_name("ESV", true).is_some());
        assert!(catalog(true, &[]).iter().any(|e| e.licensed && e.code == "NIV"));
        // installed flag reflects the input set
        assert!(catalog(true, &["KJV".into()]).iter().any(|e| e.code == "KJV" && e.installed));
    }
}
