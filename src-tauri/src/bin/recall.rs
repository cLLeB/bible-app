//! How much of a real service would we have kept up with?
//!
//! Ground truth comes free, from how a service already works: the preacher names a
//! reference, a human operator projects it, and the preacher reads it off the
//! screen. So every passage read aloud in a recording is a verse a human operator
//! got right that day.
//!
//! This finds those readings by matching the transcript against scripture, then
//! runs the transcript through the app's actual decision flow — detector, existence
//! check, quote matcher, confidence, auto-project threshold — and asks whether the
//! right verse would have been on the wall by the time he started reading.
//!
//! Usage (from src-tauri/):
//!     cargo run --release --bin recall -- <replay.log> [more.log ...]

use bible_app_lib::{audio, books, db, detect, semantic};
use std::collections::HashSet;
use std::path::Path;

/// The operator's auto-project bar (the app's default).
const AUTO_PROJECT: f32 = 0.82;

// ---- ground truth: what did he read aloud? ---------------------------------

struct Verse {
    osis: String,
    chapter: u16,
    verse: u16,
    words: HashSet<String>,
    len: usize,
}

const FILLER: &[&str] = &[
    "the", "and", "that", "for", "with", "this", "from", "have", "his", "her", "who", "was",
    "are", "but", "not", "you", "your", "they", "them", "will", "unto", "shall", "hath", "were",
    "there", "their", "which", "when", "then", "than", "into", "upon", "what", "would", "could",
    "said", "say", "says", "him", "she", "had", "has", "our", "all", "one", "out",
];

fn content_words(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(|w| w.to_lowercase())
        .filter(|w| !FILLER.contains(&w.as_str()))
        .collect()
}

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

/// Index several translations: he does not read from the one we happen to pick.
fn ground_truth_index(dir: &Path) -> Vec<Verse> {
    let mut out = Vec::new();
    for name in ["web", "kjv", "niv", "nkjv", "nlt"] {
        let Ok(json) = std::fs::read_to_string(dir.join(format!("{name}.canonical.json"))) else {
            continue;
        };
        let Ok(seed) = serde_json::from_str::<Seed>(&json) else { continue };
        for r in seed.verses {
            let words = content_words(&r.text);
            let len = words.len();
            if len >= 5 {
                out.push(Verse { osis: r.book_osis, chapter: r.chapter, verse: r.verse, words, len });
            }
        }
    }
    out
}

/// Is this utterance a *reading* — most of the verse's distinctive words present —
/// rather than a passing allusion?
fn reading_of(verses: &[Verse], text: &str) -> Option<(String, u16, u16)> {
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
    best.map(|(v, _)| (v.osis.clone(), v.chapter, v.verse))
}

// ---- the app's real decision flow ------------------------------------------

/// A scripture database with bundled translations in it, so the quote matcher runs
/// against real FTS exactly as it does in the app.
fn scripture_db(dir: &Path) -> db::Db {
    let handle = db::open_at(Path::new(":memory:")).expect("open db");
    handle.migrate().expect("migrate");
    for name in ["web", "kjv"] {
        if let Ok(json) = std::fs::read_to_string(dir.join(format!("{name}.canonical.json"))) {
            handle.seed_from_json(&json).expect("seed");
        }
    }
    handle.sync_fts().expect("fts");
    handle
}

/// What the app would put on the wall for this utterance, if anything. Mirrors
/// audio.rs: a named reference that exists wins; otherwise the words themselves are
/// matched against scripture and trusted in proportion to how much of the verse was
/// actually said.
fn decide(
    db: &db::Db,
    ctx: &mut detect::RefContext,
    showing: &Option<(String, u16, Option<u16>)>,
    text: &str,
) -> Option<(String, u16, Option<u16>, f32)> {
    let hits = detect::detect_with_context(text, ctx);
    let confident: Vec<&detect::Detection> =
        hits.iter().filter(|h| h.source != detect::DetectSource::Story).collect();

    // Mirror the app: prefer the last reference that names a verse, so a truncation
    // fragment ("…verse 4, Psalm 60—") cannot beat a complete reference.
    let chosen = confident
        .iter()
        .rev()
        .find(|d| d.reference.verse.is_some())
        .or_else(|| confident.last())
        .copied();
    if let Some(d) = chosen {
        let r = &d.reference;
        // A bare chapter mention does not knock a verse of that same chapter off the
        // screen — she says "this chapter, Romans 8" while teaching Romans 8:18.
        let bare_chapter_of_what_is_showing = r.verse.is_none()
            && showing
                .as_ref()
                .map(|(b, c, v)| *b == r.book_osis && *c == r.chapter && v.is_some())
                .unwrap_or(false);
        if bare_chapter_of_what_is_showing {
            return None;
        }
        if let Ok(Some(_)) = db.find_verse("WEB", r) {
            return Some((r.book_osis.clone(), r.chapter, r.verse, 0.95));
        }
        // Impossible reference: the remembered book is stale. Drop it and let the
        // words speak — audio.rs does the same.
        ctx.clear();
    }

    // Quoted scripture: the preacher reading the passage out.
    let (query, words) = semantic::fts_query(text)?;
    let mut found = db.search_fts("WEB", &query, 5).ok()?;
    found.extend(db.search_fts_any(&query, 5).ok()?);
    let (rec, _) = found
        .into_iter()
        .find(|(rec, _)| semantic::is_strong(semantic::overlap(&words, &rec.text), words.len()))?;
    let confidence = audio::quote_confidence(text, &rec.text);
    Some((rec.book_osis, rec.chapter, Some(rec.verse), confidence))
}

// ---- log parsing ------------------------------------------------------------

fn secs(clock: &str) -> u32 {
    let mut p = clock.split(':');
    let m: u32 = p.next().unwrap_or("0").parse().unwrap_or(0);
    let s: u32 = p.next().unwrap_or("0").parse().unwrap_or(0);
    m * 60 + s
}

fn transcripts(log: &str) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let mut pending: Option<u32> = None;
    for line in log.lines() {
        if let Some(rest) = line.strip_prefix('[') {
            let Some((at, tail)) = rest.split_once(']') else { continue };
            let t = secs(at);
            match tail.find('"') {
                Some(q) => out.push((t, tail[q + 1..].trim_end_matches('"').to_string())),
                None => pending = Some(t),
            }
        } else if let Some(at) = pending.take() {
            if let Some(text) = line.trim().strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                out.push((at, text.to_string()));
            }
        }
    }
    out
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
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("data");
    eprintln!("indexing scripture…");
    let truth = ground_truth_index(&data);
    let db = scripture_db(&data);
    eprintln!("{} verses indexed\n", truth.len());

    // Up "in time" = projected between 90s before the reading and 30s after.
    const BEFORE: u32 = 90;
    const AFTER: u32 = 30;

    let (mut read_total, mut projected_total, mut suggested_total) = (0usize, 0usize, 0usize);

    for path in &logs {
        let log = std::fs::read_to_string(path).expect("read log");
        let utts = transcripts(&log);
        let name = Path::new(path).file_stem().unwrap().to_string_lossy();

        let mut ctx = detect::RefContext::default();
        let mut wall: Vec<(u32, String, u16, Option<u16>, f32)> = Vec::new();
        // What is actually on the screen right now — only a projection changes it.
        let mut showing: Option<(String, u16, Option<u16>)> = None;
        for (at, text) in &utts {
            if let Some((osis, ch, v, conf)) = decide(&db, &mut ctx, &showing, text) {
                if conf >= AUTO_PROJECT {
                    showing = Some((osis.clone(), ch, v));
                }
                wall.push((*at, osis, ch, v, conf));
            }
        }

        let mut read: Vec<(u32, String, u16, u16)> = Vec::new();
        for (at, text) in &utts {
            if let Some((osis, ch, v)) = reading_of(&truth, text) {
                if read.iter().any(|(pt, po, pc, _)| *po == osis && *pc == ch && at - *pt < 120) {
                    continue; // one passage, read across several utterances
                }
                read.push((*at, osis, ch, v));
            }
        }

        println!("##### {name}");
        let (mut proj, mut sugg) = (0usize, 0usize);
        for (t, osis, ch, v) in &read {
            let best = wall
                .iter()
                .filter(|(pt, po, pc, pv, _)| {
                    *pt + AFTER >= *t
                        && t.saturating_sub(BEFORE) <= *pt
                        && po == osis
                        && pc == ch
                        && (pv.is_none() || *pv == Some(*v))
                })
                .map(|(_, _, _, _, c)| *c)
                .fold(0.0f32, f32::max);

            let mark = if best >= AUTO_PROJECT {
                proj += 1;
                "PROJECTED"
            } else if best > 0.0 {
                sugg += 1;
                "suggested"
            } else {
                "MISSED   "
            };
            println!("  [{:02}:{:02}] {mark}  {}", t / 60, t % 60, name_of(osis, *ch, *v));
        }
        // Coverage means nothing without the cost. Across a sermon the right answer
        // is usually "project nothing", so count everything that would have gone up.
        let auto = wall.iter().filter(|(_, _, _, _, c)| *c >= AUTO_PROJECT).count();
        let mins = utts.last().map(|(t, _)| t / 60).unwrap_or(1).max(1);
        println!(
            "  -> {proj} projected · {sugg} suggested · {} missed (of {})",
            read.len() - proj - sugg,
            read.len()
        );
        println!("     {auto} projections in total, over {mins} min of preaching");
        if std::env::var("SHOW_ALL").is_ok() {
            for (at, osis, ch, v, c) in wall.iter().filter(|(_, _, _, _, c)| *c >= AUTO_PROJECT) {
                let vs = v.unwrap_or(1);
                println!("       [{:02}:{:02}] {} ({c:.2})", at / 60, at % 60, name_of(osis, *ch, vs));
            }
        }
        println!();
        read_total += read.len();
        projected_total += proj;
        suggested_total += sugg;
    }

    let pct = |n: usize| n as f32 / read_total.max(1) as f32 * 100.0;
    println!("=== of {read_total} verses the preacher read aloud:");
    println!("    {projected_total} projected automatically ({:.0}%)", pct(projected_total));
    println!("    {suggested_total} offered as a suggestion ({:.0}%)", pct(suggested_total));
    let missed = read_total - projected_total - suggested_total;
    println!("    {missed} not found at all ({:.0}%)", pct(missed));
}
