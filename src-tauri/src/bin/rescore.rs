//! Re-score a replay log faithfully, without re-running whisper.
//!
//! `replay_sermon` prints every detection the detector makes, which overstates
//! what reaches the screen: the app also (a) looks the verse up and projects
//! nothing if it does not exist, and (b) suppresses a repeat of what is already
//! on the wall. A preacher saying "Colossians chapter 22" — Colossians has four —
//! produces a detection but no projection.
//!
//! This replays the transcripts through the real detector with those two rules
//! applied, so the numbers mean what they claim to mean.
//!
//! Usage (from src-tauri/):
//!     cargo run --release --bin rescore -- <replay.log> [more.log ...]

use bible_app_lib::{books, detect};
use std::collections::HashMap;
use std::path::Path;

type Canon = HashMap<(String, u16), u16>; // (book, chapter) -> last verse

/// Build the real canon from a bundled translation, so "does this verse exist?"
/// is answered by scripture rather than by assumption.
fn canon() -> Canon {
    #[derive(serde::Deserialize)]
    struct Seed {
        verses: Vec<Verse>,
    }
    #[derive(serde::Deserialize)]
    struct Verse {
        book_osis: String,
        chapter: u16,
        verse: u16,
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let path = root.join("data").join("web.canonical.json");
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let seed: Seed = serde_json::from_str(&json).expect("parse canon");
    let mut map: Canon = HashMap::new();
    for v in seed.verses {
        let e = map.entry((v.book_osis, v.chapter)).or_insert(0);
        *e = (*e).max(v.verse);
    }
    map
}

fn exists(canon: &Canon, osis: &str, chapter: u16, verse: Option<u16>) -> bool {
    match canon.get(&(osis.to_string(), chapter)) {
        Some(last) => verse.map(|v| v >= 1 && v <= *last).unwrap_or(true),
        None => false,
    }
}

/// Pull the utterance transcripts back out of a replay log, in order.
fn transcripts(log: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut pending_at: Option<String> = None;
    for line in log.lines() {
        if let Some(rest) = line.strip_prefix('[') {
            let Some((at, tail)) = rest.split_once(']') else { continue };
            if let Some(q) = tail.find('"') {
                let text = tail[q + 1..].trim_end_matches('"').to_string();
                out.push((at.to_string(), text));
            } else {
                pending_at = Some(at.to_string()); // detection line; transcript is next
            }
        } else if let Some(at) = pending_at.take() {
            let t = line.trim();
            if let Some(text) = t.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                out.push((at, text.to_string()));
            }
        }
    }
    out
}

fn main() {
    let logs: Vec<String> = std::env::args().skip(1).collect();
    if logs.is_empty() {
        eprintln!("usage: rescore <replay.log> [...]");
        std::process::exit(2);
    }
    let canon = canon();

    let (mut total_utt, mut total_proj, mut total_blocked) = (0usize, 0usize, 0usize);

    for path in &logs {
        let log = std::fs::read_to_string(path).expect("read log");
        let utts = transcripts(&log);
        let name = Path::new(path).file_stem().unwrap().to_string_lossy();
        println!("##### {name}  ({} utterances)", utts.len());

        let mut ctx = detect::RefContext::default();
        let mut last: Option<(String, u16, Option<u16>)> = None;
        let (mut projected, mut blocked) = (0usize, 0usize);

        for (at, text) in &utts {
            let hits = detect::detect_with_context(text, &mut ctx);
            let confident: Vec<&detect::Detection> =
                hits.iter().filter(|h| h.source != detect::DetectSource::Story).collect();
            let Some(d) = confident.last() else { continue };
            let r = &d.reference;

            if !exists(&canon, &r.book_osis, r.chapter, r.verse) {
                blocked += 1;
                println!("[{at}] (no such verse — nothing projected)   \"{text}\"");
                continue;
            }
            let key = (r.book_osis.clone(), r.chapter, r.verse);
            if last.as_ref() == Some(&key) {
                continue; // already on the wall
            }
            last = Some(key);
            projected += 1;
            let book = books::book_by_osis(&r.book_osis)
                .map(|b| b.name.to_string())
                .unwrap_or_else(|| r.book_osis.clone());
            let re = match r.verse {
                Some(v) => format!("{book} {}:{v}", r.chapter),
                None => format!("{book} {}", r.chapter),
            };
            println!("[{at}] PROJECT {re} [{:?}]\n        \"{text}\"", d.source);
        }
        println!("  -> {projected} projected · {blocked} detections blocked as impossible\n");
        total_utt += utts.len();
        total_proj += projected;
        total_blocked += blocked;
    }

    println!("=== {total_proj} projections · {total_blocked} blocked · {total_utt} utterances");
}
