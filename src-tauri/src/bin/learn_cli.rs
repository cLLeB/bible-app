//! Run the app's "learn from a recording" pipeline outside the app, so the settings
//! it derives for a preacher can be checked — and, for the preachers this church
//! actually has, baked in as the shipped defaults.
//!
//! Same code the app runs: same decoder, same segmentation, same ground truth (what
//! the preacher read aloud), same candidate settings.
//!
//! Several recordings of the same preacher are better than one: each sermon only
//! contains a handful of passages read aloud, and settings chosen on six data points
//! are settings chosen on one Sunday.
//!
//! Usage (from src-tauri/):
//!     cargo run --release --bin learn_cli -- <model> <recording> [more recordings...]

use bible_app_lib::{audio, corrections, db, learn, stt};
use std::collections::HashMap;
use std::path::Path;

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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: learn_cli <base|small> <recording> [more recordings...]");
        std::process::exit(2);
    }
    let kind = args[0].clone();
    let recordings: Vec<&String> = args[1..].iter().collect();

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let model = root.join("models").join(format!("ggml-{kind}.en.bin"));
    let binary = root.join("bin").join("whisper-cli.exe");
    let db = scripture_db(&root.join("data"));
    let baseline = stt::Decode::for_model(&model);

    // Every clip worth re-decoding, pooled across the recordings, with the verse it
    // must recover. Several sermons beat one: each carries only a handful of passages
    // read aloud, and settings picked on six data points are settings picked on one
    // Sunday.
    let mut clips: Vec<Vec<f32>> = Vec::new();
    let mut truth: Vec<(usize, String, u16)> = Vec::new(); // (index into clips, book, chapter)
    let mut learned: Vec<(String, String)> = Vec::new();
    let mut minutes = 0.0f32;

    for path in &recordings {
        let path = Path::new(path.as_str());
        eprintln!("\ndecoding {}", path.file_name().unwrap().to_string_lossy());
        let audio_all = match learn::decode_audio_file(path) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("  skipped: {e}");
                continue;
            }
        };
        minutes += audio_all.len() as f32 / 16_000.0 / 60.0;
        let utterances = audio::segment_utterances(&audio_all);
        eprintln!("  {} utterances", utterances.len());

        let mut transcripts: Vec<(usize, String)> = Vec::new();
        for (i, (_at, clip)) in utterances.iter().enumerate() {
            if i % 25 == 0 {
                eprint!("\r  listening {i}/{}   ", utterances.len());
            }
            if let Ok(raw) = stt::transcribe(clip, &model, &binary, baseline) {
                let text = corrections::correct(raw.trim());
                if !text.is_empty() {
                    transcripts.push((i, text));
                }
            }
        }
        eprintln!("\r  heard {} utterances       ", transcripts.len());

        // Whatever was read aloud is a verse a human operator got right that day.
        let mut found_here: Vec<(String, u16)> = Vec::new();
        for (i, text) in &transcripts {
            let Some(rec) = learn::reading_of(&db, text) else { continue };
            if found_here.iter().any(|(o, c)| *o == rec.book_osis && *c == rec.chapter) {
                continue; // one passage, however long the reading
            }
            found_here.push((rec.book_osis.clone(), rec.chapter));

            // Keep the reading and the utterances just before it, where the reference
            // was announced — that is the whole of what the settings have to recover.
            let first = i.saturating_sub(4);
            let base = clips.len();
            for k in first..=*i {
                if k < utterances.len() {
                    clips.push(utterances[k].1.clone());
                }
            }
            truth.push((base + (*i - first), rec.book_osis.clone(), rec.chapter));

            for (_, said) in transcripts.iter().filter(|(j, _)| *j < *i && *i - *j <= 4) {
                if let Some(word) = learn::learn_book_name(said, &rec.book_osis, rec.chapter) {
                    if !learned.iter().any(|(w, _)| *w == word) {
                        learned.push((word, rec.book_osis.clone()));
                    }
                }
            }
        }
        eprintln!("  {} passages read aloud", found_here.len());
    }

    println!("\n{:.0} min of preaching · {} passages read aloud", minutes, truth.len());
    for (_, osis, ch) in &truth {
        println!("   {osis} {ch}");
    }
    if !learned.is_empty() {
        println!("\nbook names this speaker gets misheard as:");
        for (word, osis) in &learned {
            println!("   \"{word}\" -> {osis}");
        }
    }
    if truth.is_empty() {
        eprintln!("nothing read aloud — cannot learn from these recordings");
        std::process::exit(1);
    }

    println!("\nsettings, scored on how much of that scripture they recover:");
    let mut scored: Vec<(stt::Decode, usize)> = Vec::new();
    for cfg in learn::candidates() {
        let mut texts: HashMap<usize, String> = HashMap::new();
        for (k, clip) in clips.iter().enumerate() {
            if let Ok(raw) = stt::transcribe(clip, &model, &binary, cfg) {
                texts.insert(k, corrections::correct(raw.trim()));
            }
        }
        let found = truth
            .iter()
            .filter(|(i, osis, chapter)| {
                (i.saturating_sub(4)..=*i).any(|k| {
                    texts.get(&k).map(|t| learn::resolves_to(t, osis, *chapter)).unwrap_or(false)
                })
            })
            .count();
        let label = bible_app_lib::calibrate::label_of(&cfg);
        let mark = if cfg == baseline { "   <- shipped default" } else { "" };
        println!("   {found}/{}  {label}{mark}", truth.len());
        scored.push((cfg, found));
    }

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    let (best, found) = scored[0];
    println!(
        "\nbest for this speaker: {}  ({found}/{} of what they read aloud)",
        bible_app_lib::calibrate::label_of(&best),
        truth.len()
    );
}
