//! Learn a preacher from recordings of them preaching, outside the app, so the
//! profile it derives can be checked — and, for the preachers this church actually
//! has, baked into the shipped personal installer as ready-made defaults.
//!
//! This runs the SAME `learn::run` the in-app wizard runs: same scout/target model
//! split, same ground truth (what the preacher read aloud), same candidate settings,
//! same room and translation logic. There is no second copy of the learning — a
//! profile baked from here is identical to one the app would derive from the same
//! audio. The only thing this binary adds is writing the result to a seed file.
//!
//! Usage (from src-tauri/):
//!     cargo run --release --bin learn_cli -- \
//!         <base|small> [--profile "Miss Hilda"] [--json ../profiles.seed.json] \
//!         <recording> [more recordings...]
//!
//! Several recordings of the same preacher are better than one: each sermon carries
//! only a handful of passages read aloud, and settings chosen on six data points are
//! settings chosen on one Sunday.

use bible_app_lib::{calibrate, db, learn, profile_seed};
use std::path::{Path, PathBuf};

/// A scripture DB with every bundled translation, so a reading can be matched to the
/// exact version it came from — the same library the app has when it learns.
fn scripture_db(dir: &Path) -> db::Db {
    let handle = db::open_at(Path::new(":memory:")).expect("open db");
    handle.migrate().expect("migrate");
    let mut seeded = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().map(|n| n.to_string_lossy().ends_with(".canonical.json")).unwrap_or(false)
            {
                if let Ok(json) = std::fs::read_to_string(&path) {
                    if handle.seed_from_json(&json).is_ok() {
                        seeded += 1;
                    }
                }
            }
        }
    }
    eprintln!("scripture: seeded {seeded} translation(s)");
    handle.sync_fts().expect("fts");
    handle
}

fn main() {
    // ---- arguments: <model> [--profile NAME] [--json PATH] <recordings...> -------
    let mut kind: Option<String> = None;
    let mut profile: Option<String> = None;
    let mut json_out: Option<String> = None;
    let mut recordings: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--profile" => profile = args.next(),
            "--json" => json_out = args.next(),
            _ if kind.is_none() => kind = Some(a),
            _ => recordings.push(a),
        }
    }
    let (Some(kind), false) = (kind, recordings.is_empty()) else {
        eprintln!(
            "usage: learn_cli <base|small> [--profile NAME] [--json PATH] <recording> [more...]"
        );
        std::process::exit(2);
    };

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let target_model = root.join("models").join(format!("ggml-{kind}.en.bin"));
    // Finding what was read aloud only needs the fast model — exactly as the app does.
    let scout_model = root.join("models").join("ggml-base.en.bin");
    let binary = root.join("bin").join("whisper-cli.exe");
    let db = scripture_db(&root.join("data"));

    // Progress to stderr; the model swaps once (scout -> target) as the app's does.
    let mut last = String::new();
    let say = |stage: &str, done: usize, total: usize, _base: f32, _share: f32| {
        if stage != last {
            eprintln!();
            last = stage.to_string();
        }
        eprint!("\r  {stage}: {done}/{total}        ");
    };
    let reading = |text: &str| learn::reading_of(&db, text);

    let learned = match learn::run(&scout_model, &target_model, &binary, &recordings, reading, say) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("\n{e}");
            std::process::exit(1);
        }
    };

    // ---- report ------------------------------------------------------------------
    println!(
        "\n{:.0} min of preaching · {} passages read aloud",
        learned.minutes, learned.references_found
    );
    if !learned.aliases.is_empty() {
        println!("\nbook names this speaker gets misheard as:");
        for (word, osis) in &learned.aliases {
            println!("   \"{word}\" -> {osis}");
        }
    }
    if let Some(code) = &learned.translation {
        println!("\nreads from: {code}");
    }
    println!("speech threshold measured: {:.4}", learned.room.speech_above);
    println!(
        "\nshipped default recovers {}/{}, best for this speaker recovers {}/{}:  {}",
        learned.before,
        learned.references_found,
        learned.after,
        learned.references_found,
        calibrate::label_of(&learned.decode)
    );

    // ---- bake into the seed file -------------------------------------------------
    let (Some(name), Some(out)) = (profile, json_out) else {
        eprintln!("\n(no --profile/--json: printed only, nothing written)");
        return;
    };
    let out = PathBuf::from(&out);
    let mut seed: profile_seed::Seed = std::fs::read_to_string(&out)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let model_file = target_model
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("ggml-{kind}.en.bin"));
    let entry = seed.entries.entry(name.clone()).or_default();
    entry.aliases = learned.aliases.iter().cloned().collect();
    entry.room = Some(learned.room.speech_above);
    entry.translation = learned.translation.clone();
    entry.decode.insert(model_file.clone(), profile_seed::DecodeSeed::from_decode(&learned.decode));
    // The first speaker written is the one the app should open on — the regular preacher.
    if seed.active.is_none() {
        seed.active = Some(name.clone());
    }

    match serde_json::to_string_pretty(&seed) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&out, text) {
                eprintln!("\ncould not write {}: {e}", out.display());
                std::process::exit(1);
            }
            println!("\nbaked {name} ({model_file}) into {}", out.display());
        }
        Err(e) => {
            eprintln!("\ncould not serialize seed: {e}");
            std::process::exit(1);
        }
    }
}
