//! How much of a real service would we have kept up with?
//!
//! Ground truth comes free, from how a service already works: the preacher names
//! a reference, a human operator projects it, and the preacher then reads it off
//! the screen. So every passage read aloud in a recording is a verse a human
//! operator got right that day.
//!
//! This finds those readings by matching the transcript against scripture itself,
//! then asks whether our pipeline had the same verse on the wall by the time he
//! started reading. That is the number that matters: not "did we transcribe the
//! words", but "would the right verse have been up there".
//!
//! Readings are matched against several translations, because he does not read the
//! one we happen to index — and are only accepted when the wording is close enough
//! to be a reading rather than an allusion.
//!
//! Usage (from src-tauri/):
//!     cargo run --release --bin recall -- <replay.log> [more.log ...]

use bible_app_lib::{books, detect};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Clone)]
struct Verse {
    osis: String,
    chapter: u16,
    verse: u16,
    words: HashSet<String>,
    len: usize,
}

/// Distinctive words only: the filler is shared by every verse and would make
/// everything look like a match.
const FILLER: &[&str] = &[
    "the", "and", "that", "for", "with", "this", "from", "have", "his", "her", "who", "was",
    "are", "but", "not", "you", "your", "they", "them", "will", "unto", "shall", "hath", "were",
    "there", "their", "which", "when", "then", "than", "into", "upon", "what", "would", "could",
    "said", "say", "says", "him", "she", "had", "has", "our", "all", "one", "out", "him",
];

fn content_words(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(|w| w.to_lowercase())
        .filter(|w| !FILLER.contains(&w.as_str()))
        .collect()
}

/// Index every verse of every translation we have, so a reading matches whichever
/// version he happens to be reading from.
fn index() -> Vec<Verse> {
    #[derive(serde::Deserialize)]
    struct Seed {
        verses: Vec<Row>,
    }
    #[derive(serde::Deserialize)]
    struct Row {
        book_osis: String,
        chapter: u16,
        verse: u16,
        text: String,
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data");
    let mut out = Vec::new();
    for name in ["web", "kjv", "niv", "nkjv", "nlt"] {
        let path = dir.join(format!("{name}.canonical.json"));
        let Ok(json) = std::fs::read_to_string(&path) else { continue };
        let Ok(seed) = serde_json::from_str::<Seed>(&json) else { continue };
        for r in seed.verses {
            let words = content_words(&r.text);
            let len = words.len();
            if len >= 5 {
                out.push(Verse { osis: r.book_osis, chapter: r.chapter, verse: r.verse, words, len });
            }
        }
        eprintln!("indexed {name}");
    }
    out
}

/// Is this utterance someone *reading* a verse? Requires most of the verse's
/// distinctive words to be present, so a passing allusion doesn't count.
fn reading_of(verses: &[Verse], text: &str) -> Option<(String, u16, u16, f32)> {
    let said = content_words(text);
    if said.len() < 5 {
        return None;
    }
    let mut best: Option<(&Verse, f32)> = None;
    for v in verses {
        let hit = v.words.iter().filter(|w| said.contains(*w)).count();
        let coverage = hit as f32 / v.len as f32;
        if coverage >= 0.65 && hit >= 5 && best.is_none_or(|(_, c)| coverage > c) {
            best = Some((v, coverage));
        }
    }
    best.map(|(v, c)| (v.osis.clone(), v.chapter, v.verse, c))
}

/// Would the quote matcher have found this verse from the words spoken around the
/// reading? Mirrors the app's rule: enough of the verse's distinctive words present.
fn quote_match(
    verses: &[Verse],
    utts: &[(String, String)],
    at: u32,
    osis: &str,
    chapter: u16,
    verse: u16,
) -> bool {
    let target = verses.iter().find(|v| v.osis == osis && v.chapter == chapter && v.verse == verse);
    let Some(target) = target else { return false };
    utts.iter().any(|(t, text)| {
        let ts = secs(t);
        if ts + 30 < at || ts > at + 30 {
            return false;
        }
        let said = content_words(text);
        let hit = target.words.iter().filter(|w| said.contains(*w)).count();
        hit >= 3 && (hit as f32 / target.len as f32) >= 0.5
    })
}

fn transcripts(log: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut pending: Option<String> = None;
    for line in log.lines() {
        if let Some(rest) = line.strip_prefix('[') {
            let Some((at, tail)) = rest.split_once(']') else { continue };
            match tail.find('"') {
                Some(q) => out.push((at.to_string(), tail[q + 1..].trim_end_matches('"').to_string())),
                None => pending = Some(at.to_string()),
            }
        } else if let Some(at) = pending.take() {
            let t = line.trim();
            if let Some(text) = t.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                out.push((at, text.to_string()));
            }
        }
    }
    out
}

fn secs(clock: &str) -> u32 {
    let mut p = clock.split(':');
    let m: u32 = p.next().unwrap_or("0").parse().unwrap_or(0);
    let s: u32 = p.next().unwrap_or("0").parse().unwrap_or(0);
    m * 60 + s
}

fn name_of(osis: &str, chapter: u16, verse: u16) -> String {
    let book =
        books::book_by_osis(osis).map(|b| b.name.to_string()).unwrap_or_else(|| osis.to_string());
    format!("{book} {chapter}:{verse}")
}

fn main() {
    let logs: Vec<String> = std::env::args().skip(1).collect();
    if logs.is_empty() {
        eprintln!("usage: recall <replay.log> [...]");
        std::process::exit(2);
    }
    let verses = index();
    eprintln!("{} verses indexed\n", verses.len());

    // A verse counts as "up in time" if we projected it any time from 90s before
    // the reading (he asks, the operator finds it) to 30s after (we caught up late).
    const BEFORE: u32 = 90;
    const AFTER: u32 = 30;

    let (mut total_read, mut total_hit, mut total_quote) = (0usize, 0usize, 0usize);

    for path in &logs {
        let log = std::fs::read_to_string(path).expect("read log");
        let utts = transcripts(&log);
        let name = Path::new(path).file_stem().unwrap().to_string_lossy();

        // What our pipeline would have had on the wall, and when.
        let mut ctx = detect::RefContext::default();
        let mut projections: Vec<(u32, String, u16, Option<u16>)> = Vec::new();
        for (at, text) in &utts {
            let hits = detect::detect_with_context(text, &mut ctx);
            if let Some(d) = hits.iter().filter(|h| h.source != detect::DetectSource::Story).last() {
                let r = &d.reference;
                projections.push((secs(at), r.book_osis.clone(), r.chapter, r.verse));
            }
        }

        // What the preacher actually read aloud — i.e. what a human put on screen.
        let mut read: Vec<(u32, String, u16, u16)> = Vec::new();
        for (at, text) in &utts {
            if let Some((osis, ch, v, _c)) = reading_of(&verses, text) {
                let t = secs(at);
                // A long reading spans several utterances; count the passage once.
                if read.iter().any(|(pt, po, pc, _)| *po == osis && *pc == ch && t - *pt < 120) {
                    continue;
                }
                read.push((t, osis, ch, v));
            }
        }

        println!("##### {name}");
        let mut hits = 0usize;
        let mut quote_hits = 0usize;
        for (t, osis, ch, v) in &read {
            let matched = projections.iter().any(|(pt, po, pc, pv)| {
                po == osis
                    && pc == ch
                    && (pv.is_none() || *pv == Some(*v))
                    && *pt + AFTER >= *t
                    && t.saturating_sub(BEFORE) <= *pt
            });
            let same_chapter = projections
                .iter()
                .any(|(pt, po, pc, _)| po == osis && pc == ch && *pt + AFTER >= *t && t.saturating_sub(BEFORE) <= *pt);
            // The app also matches quoted scripture by its words. That surfaces the
            // verse as a suggestion for the operator rather than projecting it
            // outright, so count it separately: it is coverage, but it needs a click.
            let quoted = quote_match(&verses, &utts, *t, osis, *ch, *v);
            if matched {
                hits += 1;
            } else if quoted {
                quote_hits += 1;
            }
            let mark = if matched {
                "HIT "
            } else if quoted {
                "QUOTE"
            } else if same_chapter {
                "near"
            } else {
                "MISS"
            };
            println!(
                "  [{:02}:{:02}] {mark} he read {}",
                t / 60,
                t % 60,
                name_of(osis, *ch, *v)
            );
        }
        println!("  -> {hits}/{} of the verses he read aloud were on the wall\n", read.len());
        total_read += read.len();
        total_hit += hits;
    }

    let pct = |n: usize| if total_read == 0 { 0.0 } else { n as f32 / total_read as f32 * 100.0 };
    println!(
        "=== of {total_read} verses read aloud: {total_hit} projected ({:.0}%),          {total_quote} surfaced from the words themselves ({:.0}%),          {} not found at all",
        pct(total_hit),
        pct(total_quote),
        total_read - total_hit - total_quote
    );
}
