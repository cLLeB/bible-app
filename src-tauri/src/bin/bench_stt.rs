//! Decode-settings bench: replay captured utterances through every combination
//! of model and decode settings, and score them on what actually matters — did
//! the right scripture come out the other end.
//!
//! Word-error rate is the wrong yardstick here. A transcript can be wrong in
//! ways the detector shrugs off ("Romans eight twenty eight" vs "romans 8:28"),
//! and right in ways it can't use. So the score is resolution accuracy: run the
//! real detection pipeline and check the top hit against ground truth.
//!
//! Usage (from src-tauri/):
//!     cargo run --release --bin bench_stt -- <captures-dir> <expected.txt>
//!
//! expected.txt: one line per utterance, in capture order.
//!     John 3:16          -> expects that book/chapter/verse
//!     1Cor 13            -> chapter-only, verse ignored
//!     nav                -> expects a navigation command, not a reference
//!     -                  -> expects no detection

use bible_app_lib::{corrections, detect, stt};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
enum Expect {
    Ref { osis: String, chapter: u16, verse: Option<u16> },
    Nav,
    None,
}

fn parse_expected(line: &str) -> Option<Expect> {
    // Editors (and PowerShell's utf8 encoder) leave a BOM on the first line.
    let line = line.trim_start_matches('\u{feff}').trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    if line == "-" {
        return Some(Expect::None);
    }
    if line.eq_ignore_ascii_case("nav") {
        return Some(Expect::Nav);
    }
    // Reuse the app's own reference parser so the ground-truth file can be
    // written the way a person would write it.
    let parsed = bible_app_lib::reference::parse_reference(line)?;
    Some(Expect::Ref { osis: parsed.book_osis, chapter: parsed.chapter, verse: parsed.verse })
}

/// Read any WAV to 16 kHz mono f32. Captures are already in that shape, but
/// accepting others lets the bench run against clips from any source.
fn read_wav(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .samples::<i32>()
            .map(|s| {
                s.map(|v| v as f32 / (1i64 << (spec.bits_per_sample - 1)) as f32)
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<_, _>>()?,
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|s| s.map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?,
    };

    let channels = spec.channels as usize;
    let mono: Vec<f32> = if channels <= 1 {
        raw
    } else {
        raw.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32).collect()
    };
    if spec.sample_rate == 16_000 {
        return Ok(mono);
    }
    let ratio = 16_000.0 / spec.sample_rate as f32;
    let out_len = (mono.len() as f32 * ratio) as usize;
    Ok((0..out_len)
        .filter_map(|i| mono.get((i as f32 / ratio) as usize).copied())
        .collect())
}

/// Mirrors audio.rs::clean_transcript — the detector never sees raw whisper output.
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

fn scores(text: &str, want: &Expect) -> bool {
    let mut ctx = detect::RefContext::default();
    let hits = detect::detect_with_context(text, &mut ctx);
    match want {
        Expect::None => hits.is_empty(),
        Expect::Nav => hits.is_empty() && detect::detect_nav_command(text).is_some(),
        Expect::Ref { osis, chapter, verse } => hits
            .first()
            .map(|h| {
                h.reference.book_osis == *osis
                    && h.reference.chapter == *chapter
                    && verse.map(|v| h.reference.verse == Some(v)).unwrap_or(true)
            })
            .unwrap_or(false),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: bench_stt <captures-dir> <expected.txt>");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[0]);
    let expected: Vec<Expect> = std::fs::read_to_string(&args[1])
        .expect("read expected.txt")
        .lines()
        .filter_map(parse_expected)
        .collect();

    let mut wavs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("read captures dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x.eq_ignore_ascii_case("wav")).unwrap_or(false))
        .collect();
    wavs.sort();

    if wavs.len() != expected.len() {
        eprintln!(
            "WARNING: {} clips but {} expected lines — scoring the overlap only",
            wavs.len(),
            expected.len()
        );
    }
    let n = wavs.len().min(expected.len());

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let binary = root.join("bin").join("whisper-cli.exe");
    if !binary.exists() {
        eprintln!("no whisper binary at {}", binary.display());
        std::process::exit(2);
    }
    let models: Vec<(&str, PathBuf)> = ["base", "small"]
        .iter()
        .map(|m| (*m, root.join("models").join(format!("ggml-{m}.en.bin"))))
        .filter(|(_, p)| p.exists())
        .collect();

    // Encode dominates runtime, so fit_window is swept first-class alongside the
    // accuracy knobs — it is the setting that decides whether beam search is
    // affordable at all. BENCH_GRID=quick trims to the four that matter when the
    // point is speed rather than an accuracy shoot-out.
    // The window margin is the knob that trades encode time against decoder
    // loops/accuracy, so it is swept as a value rather than a flag.
    let quick = std::env::var("BENCH_GRID").map(|v| v == "quick").unwrap_or(false);
    let windows = if quick {
        vec![stt::Window::Fit { margin: 1.5 }, stt::Window::Full]
    } else {
        vec![
            stt::Window::Fit { margin: 1.2 },
            stt::Window::Fit { margin: 1.5 },
            stt::Window::Fit { margin: 2.0 },
            stt::Window::Fit { margin: 3.0 },
            stt::Window::Full,
        ]
    };
    let mut configs: Vec<stt::Decode> = Vec::new();
    for beam in [1, 5] {
        for window in &windows {
            configs.push(stt::Decode { beam, prompt: true, normalize: true, window: *window });
        }
    }
    if !quick {
        // Controls: is the scripture prompt earning its keep, and does gain matter?
        let base = stt::Decode { beam: 5, window: stt::Window::Fit { margin: 1.5 }, ..Default::default() };
        configs.push(stt::Decode { prompt: false, ..base });
        configs.push(stt::Decode { normalize: false, ..base });
    }

    println!("{n} clips · {} models · {} configs\n", models.len(), configs.len());

    let mut summary: Vec<(String, usize, f32)> = Vec::new();
    for (name, model) in &models {
        for cfg in &configs {
            let win = match cfg.window {
                stt::Window::Full => "full".to_string(),
                stt::Window::Fit { margin } => format!("fit{margin}"),
            };
            let label = format!(
                "{name} beam={} prompt={} norm={} win={win}",
                cfg.beam, cfg.prompt as u8, cfg.normalize as u8
            );
            let mut correct = 0usize;
            let mut secs = 0.0f32;
            println!("--- {label}");
            for i in 0..n {
                let audio = match read_wav(&wavs[i]) {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("  skip {}: {e}", wavs[i].display());
                        continue;
                    }
                };
                let t0 = Instant::now();
                let raw = match stt::transcribe(&audio, model, &binary, *cfg) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("  {i:>2} FAILED: {e}");
                        continue;
                    }
                };
                secs += t0.elapsed().as_secs_f32();
                let text = clean(&raw);
                let ok = scores(&text, &expected[i]);
                if ok {
                    correct += 1;
                }
                println!("  {i:>2} {} {text}", if ok { "OK  " } else { "MISS" });
            }
            let pct = correct as f32 / n as f32 * 100.0;
            println!("  => {correct}/{n} resolved ({pct:.0}%) · {:.2}s/clip\n", secs / n as f32);
            summary.push((label, correct, secs / n as f32));
        }
    }

    summary.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.total_cmp(&b.2)));
    println!("=== ranked (accuracy first, then speed)");
    for (label, correct, per) in &summary {
        println!("  {correct:>2}/{n}  {per:>5.2}s  {label}");
    }
}
