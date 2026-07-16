//! Dump the transcribed sentences of a recording, so a learned alias can be judged in
//! context — e.g. what was actually said around "elves" before a Proverbs reading.
//! Same base decode the discovery pass uses. Reads nothing but the audio.
//!
//! Usage (from src-tauri/):
//!   cargo run --release --bin dump_cli -- [--mark elves] <recording> [more...]

use bible_app_lib::{audio, corrections, learn, stt};
use std::path::Path;

fn main() {
    let mut mark: Option<String> = None;
    let mut recordings: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--mark" => mark = args.next().map(|s| s.to_lowercase()),
            _ => recordings.push(a),
        }
    }
    if recordings.is_empty() {
        eprintln!("usage: dump_cli [--mark WORD] <recording> [more...]");
        std::process::exit(2);
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let model = root.join("models").join("ggml-base.en.bin");
    let binary = root.join("bin").join("whisper-cli.exe");
    let decode = stt::Decode::for_model(&model);

    for path in &recordings {
        let p = Path::new(path);
        println!("\n########## {} ##########", p.file_name().unwrap().to_string_lossy());
        let audio_all = match learn::decode_audio_file(p) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("  skipped: {e}");
                continue;
            }
        };
        let utterances = audio::segment_utterances(&audio_all);
        for (i, (_at, clip)) in utterances.iter().enumerate() {
            if let Ok(raw) = stt::transcribe(clip, &model, &binary, decode) {
                let text = corrections::correct(raw.trim());
                if text.is_empty() {
                    continue;
                }
                let hit = mark.as_ref().map(|m| text.to_lowercase().contains(m.as_str())).unwrap_or(false);
                let flag = if hit { "  <<<<< MARK" } else { "" };
                println!("[{i:>4}] {text}{flag}");
            }
        }
    }
}
