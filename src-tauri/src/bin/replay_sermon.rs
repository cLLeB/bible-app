//! Replay a recorded sermon through the real listening pipeline.
//!
//! The calibration script tests whether we can hear a speaker reading lines. This
//! tests the thing that actually happens on a Sunday: a preacher talking for forty
//! minutes, with references buried in sentences, and long stretches where the
//! right behaviour is to project *nothing at all*.
//!
//! So it reports two numbers, and the second one matters more:
//!   * what it detected — did the real references get caught?
//!   * how often it detected anything — every false hit is a wrong verse thrown
//!     on the wall mid-sermon.
//!
//! Usage (from src-tauri/):
//!     cargo run --release --bin replay_sermon -- <wav> [base|small|tiny]

use bible_app_lib::{audio, books, corrections, detect, stt};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn read_wav_16k(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    if spec.sample_rate != 16_000 || spec.channels != 1 {
        return Err(format!("{}: need 16 kHz mono", path.display()));
    }
    reader
        .samples::<i16>()
        .map(|s| s.map(|v| v as f32 / i16::MAX as f32).map_err(|e| e.to_string()))
        .collect()
}

fn clean(raw: &str) -> String {
    let joined = raw
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| !(l.starts_with('[') && l.ends_with(']')))
        .collect::<Vec<_>>()
        .join(" ");
    corrections::correct(&joined.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn clock(secs: f32) -> String {
    format!("{:02}:{:02}", (secs as u32) / 60, (secs as u32) % 60)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: replay_sermon <wav> [base|small|tiny]");
        std::process::exit(2);
    }
    let wav = PathBuf::from(&args[0]);
    let kind = args.get(1).cloned().unwrap_or_else(|| "base".to_string());

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let model = root.join("models").join(format!("ggml-{kind}.en.bin"));
    let binary = root.join("bin").join("whisper-cli.exe");
    let decode = stt::Decode::for_model(&model);

    let audio_all = read_wav_16k(&wav).expect("read wav");
    let minutes = audio_all.len() as f32 / 16_000.0 / 60.0;
    let utterances = audio::segment_utterances(&audio_all);

    println!("{}", wav.file_name().unwrap().to_string_lossy());
    println!(
        "{minutes:.0} min · {} utterances · model {kind} · {:?}\n",
        utterances.len(),
        decode.window
    );

    let mut ctx = detect::RefContext::default();
    let mut detections = 0usize;
    let mut with_verse = 0usize;
    let t0 = Instant::now();

    for (at, samples) in &utterances {
        let raw = match stt::transcribe(samples, &model, &binary, decode) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[{}] transcribe failed: {e}", clock(*at));
                continue;
            }
        };
        let text = clean(&raw);
        if text.is_empty() {
            continue;
        }
        let hits = detect::detect_with_context(&text, &mut ctx);
        if hits.is_empty() {
            // Print the quiet utterances too. A detector is judged as much by the
            // forty minutes it stays silent through as by what it catches, and the
            // misses only show up against the full transcript.
            println!("[{}] -    \"{text}\"", clock(*at));
            continue;
        }
        detections += hits.len();
        let show = |h: &detect::Detection| {
            let book = books::book_by_osis(&h.reference.book_osis)
                .map(|b| b.name.to_string())
                .unwrap_or_else(|| h.reference.book_osis.clone());
            match h.reference.verse {
                Some(v) => format!("{book} {}:{v} [{:?}]", h.reference.chapter, h.source),
                None => format!("{book} {} [{:?}]", h.reference.chapter, h.source),
            }
        };

        // Mirror what the app puts on the wall: the last confident reference in
        // the utterance wins (speakers correct themselves), the rest are offered
        // as alternatives. Story-only hits sit below the auto-project threshold,
        // so they are a suggestion, never a projection.
        let confident: Vec<&detect::Detection> =
            hits.iter().filter(|h| h.source != detect::DetectSource::Story).collect();
        match confident.last() {
            Some(primary) => {
                if primary.reference.verse.is_some() {
                    with_verse += 1;
                }
                let alts: Vec<String> =
                    confident.iter().rev().skip(1).map(|h| show(h)).collect();
                let tail =
                    if alts.is_empty() { String::new() } else { format!("   (alt: {})", alts.join(", ")) };
                println!("[{}] PROJECT {}{tail}", clock(*at), show(primary));
            }
            None => {
                let stories: Vec<String> = hits.iter().map(show).collect();
                println!("[{}] suggest {}", clock(*at), stories.join(", "));
            }
        }
        println!("        \"{text}\"");
    }

    let mins = t0.elapsed().as_secs_f32() / 60.0;
    println!(
        "\n{} detections ({with_verse} with a verse) across {} utterances of {minutes:.0} min speech",
        detections,
        utterances.len()
    );
    println!("replayed in {mins:.1} min");
}
