//! Measure the number that actually matters on a Sunday: of the scriptures a speaker
//! DELIBERATELY calls for — flagged by a lead-in ("let's turn our Bibles to…") and/or
//! spoken in full ("John chapter 3 verse 16") — how many would the app catch and put
//! on screen live, with that speaker's tuned settings.
//!
//! It reuses the exact discovery the learning uses (fast base model finds what was read
//! aloud), then for each passage classifies the announcement — did she cue it? did she
//! speak the full book+chapter+verse? — and checks whether her tuned settings resolve
//! the reference. It reads the learned profile but NEVER changes it: pure measurement.
//!
//! Usage (from src-tauri/):
//!   cargo run --release --bin measure_cli -- --profile "Miss Hilda" \
//!       --seed ../profiles.seed.json --out ../learning-results/miss-hilda-measure.json \
//!       ../sermons/president/*.mp3

use bible_app_lib::{audio, corrections, db, detect, learn, profile_seed, stt};
use std::path::{Path, PathBuf};

fn scripture_db(dir: &Path) -> db::Db {
    let handle = db::open_at(Path::new(":memory:")).expect("open db");
    handle.migrate().expect("migrate");
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.file_name().map(|n| n.to_string_lossy().ends_with(".canonical.json")).unwrap_or(false) {
                if let Ok(json) = std::fs::read_to_string(&p) {
                    let _ = handle.seed_from_json(&json);
                }
            }
        }
    }
    handle.sync_fts().expect("fts");
    handle
}

/// This speaker's tuned decode for a model, from the seed; shipped default if absent.
fn tuned(seed: &Option<profile_seed::Seed>, profile: &str, model_file: &str, model: &Path) -> stt::Decode {
    seed.as_ref()
        .and_then(|s| s.entries.get(profile))
        .and_then(|e| e.decode.get(model_file))
        .map(|d| d.to_decode())
        .unwrap_or_else(|| stt::Decode::for_model(model))
}

/// Lead-in phrases that signal "I want this on the screen now."
const CUES: &[&str] = &[
    "turn", "open your", "open our", "go to", "look at", "let's read", "let us read",
    "our bible", "your bible", "with me to", "found in", "over in", "let's go to", "let us go to",
];

fn has_cue(texts: &[String]) -> bool {
    texts.iter().any(|t| {
        let l = t.to_lowercase();
        CUES.iter().any(|c| l.contains(c))
    })
}

/// Was the full book+chapter+verse actually spoken (not just the book, or context)?
fn full_reference_spoken(texts: &[String], osis: &str, chapter: u16) -> bool {
    texts.iter().any(|t| {
        let mut ctx = detect::RefContext::default();
        detect::detect_with_context(t, &mut ctx).iter().any(|h| {
            h.reference.book_osis == osis && h.reference.chapter == chapter && h.reference.verse.is_some()
        })
    })
}

fn main() {
    let mut profile = String::from("Miss Hilda");
    let mut seed_path: Option<String> = None;
    let mut out: Option<String> = None;
    let mut recordings: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--profile" => profile = args.next().unwrap_or(profile),
            "--seed" => seed_path = args.next(),
            "--out" => out = args.next(),
            _ => recordings.push(a),
        }
    }
    if recordings.is_empty() {
        eprintln!("usage: measure_cli --profile NAME --seed seed.json --out report.json <recordings...>");
        std::process::exit(2);
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let scout_model = root.join("models").join("ggml-small.en.bin");
    let base_model = root.join("models").join("ggml-small.en.bin");
    let small_model = root.join("models").join("ggml-small.en.bin");
    let binary = root.join("bin").join("whisper-cli.exe");
    let db = scripture_db(&root.join("data"));
    let scout = stt::Decode::for_model(&scout_model);

    let seed: Option<profile_seed::Seed> = seed_path
        .and_then(|p| std::fs::read_to_string(&p).ok())
        .and_then(|s| serde_json::from_str(&s).ok());
    let base_tuned = tuned(&seed, &profile, "ggml-small.en.bin", &base_model);
    let small_tuned = tuned(&seed, &profile, "ggml-small.en.bin", &small_model);

    // Per-passage record.
    struct Passage {
        osis: String,
        chapter: u16,
        cue: bool,
        full: bool,
        caught_base: bool,
        caught_small: bool,
    }
    let mut passages: Vec<Passage> = Vec::new();

    for path in &recordings {
        let p = Path::new(path);
        eprintln!("\ndecoding {}", p.file_name().unwrap().to_string_lossy());
        let audio_all = match learn::decode_audio_file(p) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("  skipped: {e}");
                continue;
            }
        };
        let utterances = audio::segment_utterances(&audio_all);
        // Scout transcripts (fast base) to find what was read and read the announcements.
        let mut transcripts: Vec<(usize, String)> = Vec::new();
        for (i, (_at, clip)) in utterances.iter().enumerate() {
            if i % 25 == 0 {
                eprint!("\r  listening {i}/{}   ", utterances.len());
            }
            if let Ok(raw) = stt::transcribe(clip, &scout_model, &binary, scout) {
                let text = corrections::correct(raw.trim());
                if !text.is_empty() {
                    transcripts.push((i, text));
                }
            }
        }
        eprintln!("\r  heard {} utterances        ", transcripts.len());

        let mut seen: Vec<(String, u16)> = Vec::new();
        for (i, text) in &transcripts {
            let Some(rec) = learn::reading_of(&db, text) else { continue };
            if seen.iter().any(|(o, c)| *o == rec.book_osis && *c == rec.chapter) {
                continue;
            }
            seen.push((rec.book_osis.clone(), rec.chapter));

            // The announcement window: the reading and the utterances just before it.
            let first = i.saturating_sub(4);
            let window_texts: Vec<String> = transcripts
                .iter()
                .filter(|(j, _)| *j >= first && *j <= *i)
                .map(|(_, t)| t.clone())
                .collect();
            let window_clips: Vec<&Vec<f32>> = (first..=*i)
                .filter_map(|k| utterances.get(k).map(|(_, c)| c))
                .collect();

            let cue = has_cue(&window_texts);
            let full = full_reference_spoken(&window_texts, &rec.book_osis, rec.chapter);

            // Would her tuned settings resolve the reference from the announcement?
            let caught = |model: &Path, decode: stt::Decode| -> bool {
                window_clips.iter().any(|clip| {
                    stt::transcribe(clip, model, &binary, decode)
                        .map(|raw| learn::resolves_to(&corrections::correct(raw.trim()), &rec.book_osis, rec.chapter))
                        .unwrap_or(false)
                })
            };
            let caught_base = caught(&base_model, base_tuned);
            let caught_small = caught(&small_model, small_tuned);

            passages.push(Passage {
                osis: rec.book_osis.clone(),
                chapter: rec.chapter,
                cue,
                full,
                caught_base,
                caught_small,
            });
        }
    }

    // ---- tally --------------------------------------------------------------------
    let total = passages.len();
    let deliberate: Vec<&Passage> = passages.iter().filter(|p| p.cue || p.full).collect();
    let full_only: Vec<&Passage> = passages.iter().filter(|p| p.full).collect();
    let nd = deliberate.len();
    let count = |v: &[&Passage], f: &dyn Fn(&Passage) -> bool| v.iter().filter(|p| f(p)).count();

    println!("\n=== {profile}: what she deliberately calls for ===");
    println!("passages read aloud (any kind):      {total}");
    println!("  with a lead-in cue:                {}", count(&passages.iter().collect::<Vec<_>>(), &|p| p.cue));
    println!("  full book+chapter+verse spoken:    {}", full_only.len());
    println!("  deliberate (cue and/or full):      {nd}");
    println!();
    println!("caught live (reference resolved), of the {nd} deliberate:");
    println!("  base model (her base settings):    {}/{nd}", count(&deliberate, &|p| p.caught_base));
    println!("  small model (her small settings):  {}/{nd}", count(&deliberate, &|p| p.caught_small));
    println!();
    println!("caught live, of ALL {total} passages:");
    println!("  base:  {}/{total}    small: {}/{total}",
        count(&passages.iter().collect::<Vec<_>>(), &|p| p.caught_base),
        count(&passages.iter().collect::<Vec<_>>(), &|p| p.caught_small));

    if let Some(out) = out {
        let rows: Vec<serde_json::Value> = passages.iter().map(|p| serde_json::json!({
            "osis": p.osis, "chapter": p.chapter, "cue": p.cue, "full_reference": p.full,
            "caught_base": p.caught_base, "caught_small": p.caught_small,
        })).collect();
        let report = serde_json::json!({
            "profile": profile,
            "total_passages": total,
            "deliberate": nd,
            "full_reference_spoken": full_only.len(),
            "deliberate_caught_base": count(&deliberate, &|p| p.caught_base),
            "deliberate_caught_small": count(&deliberate, &|p| p.caught_small),
            "all_caught_base": count(&passages.iter().collect::<Vec<_>>(), &|p| p.caught_base),
            "all_caught_small": count(&passages.iter().collect::<Vec<_>>(), &|p| p.caught_small),
            "passages": rows,
        });
        if let Ok(text) = serde_json::to_string_pretty(&report) {
            let _ = std::fs::write(PathBuf::from(&out), text);
            println!("\nreport written to {out}");
        }
    }
}
