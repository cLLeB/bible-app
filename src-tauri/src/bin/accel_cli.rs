//! What this machine can run whisper on, and how fast.
//!
//! The same measurement the app takes from its settings screen, without the app —
//! so a backend can be checked on a church's actual laptop over a remote session,
//! and so the probe itself can be developed without clicking through a GUI.
//!
//! Usage (from src-tauri/):
//!     cargo run --release --bin accel_cli                 # list what is available
//!     cargo run --release --bin accel_cli -- --measure    # time each of them
//!     cargo run --release --bin accel_cli -- --measure --clip speech.wav
//!
//! `--clip` points the measurement at a real recording of speech, which is worth
//! far more than the synthetic fallback: the synthetic clip contains no words, and
//! whisper answers that by looping, which distorts the very comparison being made.

use bible_app_lib::{accel, accel_probe};
use std::path::{Path, PathBuf};

fn bin_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(|p| p.join("bin")).unwrap_or_default()
}

fn a_model() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.join("models");
    // Whichever is present; the comparison between backends holds for any of them.
    ["small", "base", "medium", "tiny"]
        .iter()
        .map(|k| dir.join(format!("ggml-{k}.en.bin")))
        .find(|p| p.exists())
}

fn main() {
    let root = bin_root();
    println!("whisper builds in {}", root.display());
    for b in accel::Backend::RANKED {
        let installed = accel::dir_for(&root, b).is_some();
        let usable = accel::available(&root).contains(&b);
        let note = match (installed, usable) {
            (false, _) => "not installed (scripts/fetch_whisper_backends.py)",
            (true, false) => "installed, but this machine's drivers cannot run it",
            (true, true) => "ready",
        };
        println!("  {:<28} {}", b.label(), note);
    }

    let argv: Vec<String> = std::env::args().collect();

    // --verify: the same check the app runs by itself at startup. "ready" above only
    // means the build is on disk and its driver loads; this asks the backend to
    // actually transcribe, which is the difference between a graphics card that is
    // installed and one that works.
    if argv.iter().any(|a| a == "--verify") {
        let Some(model) = a_model() else {
            eprintln!("\nNo whisper model in models/ to verify with.");
            std::process::exit(1);
        };
        println!("\nProving each one can transcribe:");
        let usable = accel::available(&root);
        for b in accel::Backend::RANKED {
            let Some(dir) = accel::dir_for(&root, b) else { continue };
            // Skipped rather than tested when the drivers rule it out: the app would
            // never choose it either, and a GPU build with no card behind it quietly
            // falls back to the processor and would otherwise report "works".
            if !usable.contains(&b) {
                println!("  {:<28} skipped - no driver on this machine", b.label());
                continue;
            }
            let ok = accel_probe::smoke_test(&dir, &model, b);
            println!("  {:<28} {}", b.label(), if ok { "works" } else { "FAILED" });
        }
        return;
    }

    if !argv.iter().any(|a| a == "--measure") {
        println!("\nPass --measure to time them, or --verify to check they work.");
        return;
    }

    let Some(model) = a_model() else {
        eprintln!("\nNo whisper model in models/ to measure with.");
        std::process::exit(1);
    };
    println!("\nMeasuring with {}", model.file_name().unwrap_or_default().to_string_lossy());
    println!("(milliseconds of compute per utterance; model loading excluded)\n");

    // --clip <path>
    let args = &argv;
    let clip: Option<PathBuf> = args
        .iter()
        .position(|a| a == "--clip")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    if let Some(c) = &clip {
        println!("Timing against {}", c.display());
    }

    match accel_probe::measure(&root, &model, clip.as_deref(), None, |t| {
        println!("  {:<28} {:>2} threads   {:>6} ms", t.backend.label(), t.threads, t.ms);
    }) {
        Ok(measured) => match measured.best() {
            Some(b) => {
                println!(
                    "\nFastest here: {} at {} threads ({} ms).",
                    b.backend.label(),
                    b.threads,
                    b.ms
                );
                if !measured.real_audio {
                    println!(
                        "\nNote: no captured utterance was available, so this ran on synthetic\n\
                         audio. It ranks the processors, but the milliseconds are not what a\n\
                         sermon will cost. Capture an utterance (calibration) and run it again."
                    );
                }
            }
            None => eprintln!("\nNo trial completed."),
        },
        Err(e) => {
            eprintln!("\n{e}");
            std::process::exit(1);
        }
    }
}
