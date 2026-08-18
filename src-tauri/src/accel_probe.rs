//! Measuring this machine, rather than assuming things about laptops in general.
//!
//! The ranking in `accel` says a graphics card beats a processor, which holds on the
//! hardware measured so far but is still a claim about laptops in general rather than
//! about the one in the sound booth. Integrated graphics was expected to be the
//! awkward case; on a 15W Iris Xe it won by about five times. That is a reason to keep
//! measuring, not to stop: the number will not be five everywhere, and a machine where
//! the ranking is wrong is exactly the machine that can least afford it.
//!
//! The probe times `whisper-cli` and reads the model's own timing report rather than
//! the wall clock, so loading half a gigabyte of weights is left out of the number.
//! Loading happens once per service; what is being compared here is the cost of one
//! utterance, which is paid over and over.
//!
//! It is deliberately not run during a service. Six or seven trial transcriptions
//! compete with the very thing they are trying to make faster.

use crate::accel::Backend;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// One trial: what was tried, and what a single utterance cost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trial {
    pub backend: Backend,
    pub threads: usize,
    /// Compute for one pass, in milliseconds. Model loading excluded.
    pub ms: u64,
}

/// The timing whisper reports for itself, summed over the parts that recur per
/// utterance. `load time` is left out on purpose (paid once per service) and so is
/// `total time` (which includes it).
fn compute_ms(stderr: &str) -> Option<u64> {
    const PARTS: [&str; 4] = ["encode time", "decode time", "batchd time", "prompt time"];
    let mut total = 0.0f64;
    let mut seen = false;
    for line in stderr.lines() {
        let Some((label, rest)) = line.split_once('=') else { continue };
        if !PARTS.iter().any(|p| label.contains(p)) {
            continue;
        }
        // "  5183.09 ms /     1 runs ( ... )" — the first number is the total for
        // that part, which is what we want; the per-run figure in brackets is not.
        let Some(ms) = rest.split_whitespace().next().and_then(|n| n.parse::<f64>().ok()) else {
            continue;
        };
        total += ms;
        seen = true;
    }
    seen.then(|| total.round() as u64)
}

/// How many times each candidate is timed. See `best_of`.
const REPS: usize = 2;

/// Speech-shaped audio to time against, used only when no real utterance has been
/// captured from this room.
///
/// Do not trust this clip as far as a real one. The model finds no words in it, and
/// with beam search it answers by looping: measured here, decode on this clip grew
/// until it rivalled the encoder, which is not how a real utterance behaves and which
/// muddies the very comparison the probe exists to make. `Measured::real_audio` says
/// which kind of clip was used, so the answer can be presented for what it is.
fn synthetic_clip() -> Vec<f32> {
    const SECS: usize = 4;
    let n = crate::audio::TARGET_RATE as usize * SECS;
    (0..n)
        .map(|i| {
            let t = i as f32 / crate::audio::TARGET_RATE as f32;
            // A pitch, two formants, and a syllable rate. Enough shape that the
            // model attempts words rather than declaring the clip blank.
            let envelope = 0.5 * (1.0 + (std::f32::consts::TAU * 4.0 * t).sin());
            let voice = (std::f32::consts::TAU * 130.0 * t).sin() * 0.5
                + (std::f32::consts::TAU * 700.0 * t).sin() * 0.3
                + (std::f32::consts::TAU * 2400.0 * t).sin() * 0.15;
            voice * envelope * 0.6
        })
        .collect()
}

/// Read a 16 kHz mono WAV to samples.
fn read_wav(path: &Path) -> Option<Vec<f32>> {
    let mut reader = hound::WavReader::open(path).ok()?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().flatten().collect(),
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().flatten().map(|v| v as f32 / scale).collect()
        }
    };
    (!samples.is_empty()).then_some(samples)
}

/// The newest real utterance this machine has captured, if any. Preferred over
/// synthetic audio: it exercises the decoder the way a sermon does.
fn captured_clip(captures: Option<&Path>) -> Option<Vec<f32>> {
    let mut wavs: Vec<PathBuf> = std::fs::read_dir(captures?)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x.eq_ignore_ascii_case("wav")).unwrap_or(false))
        .collect();
    wavs.sort();
    read_wav(wavs.last()?)
}

/// Run one trial. Returns None if that backend cannot run here at all, which is a
/// perfectly ordinary outcome: a build may ship a CUDA backend to a machine with an
/// AMD card in it.
///
/// Decodes with the settings a real service uses, via `stt::cli_decode_args`. An
/// earlier version left them out and so encoded whisper's full 30-second window on a
/// four-second clip — four times the work, and skewed towards the encoder, which is
/// the half a graphics card helps most with. It would have flattered the GPU.
fn trial(
    bin_dir: &Path,
    model: &Path,
    wav: &Path,
    secs: f32,
    backend: Backend,
    threads: usize,
) -> Option<Trial> {
    let exe = bin_dir.join(if cfg!(windows) { "whisper-cli.exe" } else { "whisper-cli" });
    let mut cmd = Command::new(&exe);
    cmd.args([
        "-m",
        model.to_str()?,
        "-f",
        wav.to_str()?,
        "-l",
        "en",
        "-t",
        &threads.to_string(),
        "-nt",
        // No output file: the transcript is irrelevant, only the timings matter.
    ]);
    cmd.args(crate::stt::cli_decode_args(crate::stt::Decode::for_model(model), secs));
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().ok()?;
    let ms = compute_ms(&String::from_utf8_lossy(&out.stderr))?;
    Some(Trial { backend, threads, ms })
}

/// Can this backend transcribe at all on this machine?
///
/// The question is not how fast but whether it works, so it is deliberately the
/// cheapest thing that still exercises the whole path: load the model onto the
/// device, encode, decode, report timings. A backend whose driver loads but whose GPU
/// cannot run whisper's shaders fails right here, at launch, instead of in the middle
/// of a service.
///
/// One second of audio and the smallest encoder window allowed, because none of the
/// numbers are kept — only whether it survived.
pub fn smoke_test(bin_dir: &Path, model: &Path, backend: Backend) -> bool {
    let clip: Vec<f32> = synthetic_clip().into_iter().take(crate::audio::TARGET_RATE as usize).collect();
    let wav = std::env::temp_dir().join(format!("bibleapp_smoke_{}.wav", std::process::id()));
    if crate::stt::write_wav_16k_mono(&wav, &clip).is_err() {
        return false;
    }
    let exe = bin_dir.join(if cfg!(windows) { "whisper-cli.exe" } else { "whisper-cli" });
    let mut cmd = Command::new(&exe);
    let (Some(m), Some(w)) = (model.to_str(), wav.to_str()) else { return false };
    cmd.args(["-m", m, "-f", w, "-l", "en", "-t", "2", "-nt", "-ac", "256", "-bs", "1", "-bo", "1"]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output();
    let _ = std::fs::remove_file(&wav);
    let Ok(o) = out else { return false };
    let stderr = String::from_utf8_lossy(&o.stderr);
    // Timings only appear when a pass actually completed, so their presence is the
    // test that it ran at all. An exit code alone is not enough: a backend can print
    // an error, fall back internally and still exit zero.
    if !o.status.success() || compute_ms(&stderr).is_none() {
        return false;
    }
    if backend == Backend::Cpu {
        return true;
    }
    // For a graphics backend, "did it run" is the wrong question — "did it run on the
    // graphics card" is the right one. The CUDA build was seen transcribing quite
    // happily on a machine with no NVIDIA card in it, by quietly falling back to the
    // processor. Verifying that as a working GPU would mean believing we had
    // acceleration we were not getting, and never finding out. So the device whisper
    // names for itself is the proof.
    stderr.lines().any(|l| crate::stt::parse_device(l).is_some())
}

/// Time one candidate more than once and keep the best.
///
/// A single reading is not worth much. This laptop was 69% busy with a browser
/// during development and the ranking flipped between runs on that alone; taking the
/// fastest of a few passes is the standard way to see the machine rather than
/// whatever else it happened to be doing.
fn best_of(
    bin_dir: &Path,
    model: &Path,
    wav: &Path,
    secs: f32,
    backend: Backend,
    threads: usize,
    reps: usize,
) -> Option<Trial> {
    (0..reps)
        .filter_map(|_| trial(bin_dir, model, wav, secs, backend, threads))
        .min_by_key(|t| t.ms)
}

/// Time every backend this machine can run, then sweep the thread count on the
/// winner. Sweeping threads on all of them would multiply the trials for an answer
/// that only matters on whichever one is actually going to be used.
///
/// `report` is called after each trial so a long measurement can show progress
/// rather than appearing to hang.
/// `clip` names a specific WAV to time against, which beats everything else when
/// one is to hand — any recording of real speech will do. Otherwise the newest
/// captured utterance, and failing that the synthetic fallback.
pub fn measure(
    bin_root: &Path,
    model: &Path,
    clip: Option<&Path>,
    captures: Option<&Path>,
    mut report: impl FnMut(&Trial),
) -> Result<Measured, String> {
    let backends = crate::accel::available(bin_root);
    if backends.is_empty() {
        return Err("No whisper build was found to measure.".into());
    }
    let real = clip.and_then(read_wav).or_else(|| captured_clip(captures));
    let real_audio = real.is_some();
    let clip = real.unwrap_or_else(synthetic_clip);
    let secs = clip.len() as f32 / crate::audio::TARGET_RATE as f32;
    let wav = std::env::temp_dir().join(format!("bibleapp_probe_{}.wav", std::process::id()));
    crate::stt::write_wav_16k_mono(&wav, &clip)?;

    let base_threads: usize = crate::stt::threads().parse().unwrap_or(4);
    let mut results: Vec<Trial> = Vec::new();

    for b in &backends {
        let Some(dir) = crate::accel::dir_for(bin_root, *b) else { continue };
        if let Some(t) = best_of(&dir, model, &wav, secs, *b, base_threads, REPS) {
            report(&t);
            results.push(t);
        }
    }

    // The thread count matters most on the CPU, where ggml's per-layer barrier means
    // a pass runs at the speed of its slowest thread. It is swept on whatever won, so
    // a machine that ends up on the CPU still gets the benefit.
    if let Some(best) = results.iter().min_by_key(|t| t.ms).copied() {
        if let Some(dir) = crate::accel::dir_for(bin_root, best.backend) {
            for n in crate::stt::thread_candidates() {
                if n == best.threads {
                    continue;
                }
                if let Some(t) = best_of(&dir, model, &wav, secs, best.backend, n, REPS) {
                    report(&t);
                    results.push(t);
                }
            }
        }
    }

    let _ = std::fs::remove_file(&wav);
    if results.is_empty() {
        return Err("No backend completed a trial transcription.".into());
    }
    Ok(Measured { trials: results, real_audio })
}

/// The result of a measurement, and how much it is worth.
pub struct Measured {
    pub trials: Vec<Trial>,
    /// True when a real captured utterance was timed. False means the synthetic
    /// fallback was used, which ranks backends roughly but should not be read as a
    /// precise figure — see `synthetic_clip`.
    pub real_audio: bool,
}

impl Measured {
    /// The winning trial: fewest milliseconds per utterance.
    pub fn best(&self) -> Option<Trial> {
        self.trials.iter().min_by_key(|t| t.ms).copied()
    }

    /// The fastest time seen for each backend, so a settings screen can show why the
    /// winner won rather than only announcing it.
    pub fn by_backend(&self) -> std::collections::HashMap<String, u64> {
        let mut out: std::collections::HashMap<String, u64> = Default::default();
        for t in &self.trials {
            let e = out.entry(t.backend.key().into()).or_insert(t.ms);
            *e = (*e).min(t.ms);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_recurring_costs_and_ignores_loading() {
        // Real output, from this laptop. Loading is paid once a service; the rest is
        // paid on every single thing the preacher says.
        let stderr = "\
whisper_print_timings:     load time =  3899.20 ms
whisper_print_timings:   encode time =  5183.09 ms /     1 runs (  5183.09 ms per run)
whisper_print_timings:   decode time =    75.94 ms /     2 runs (    37.97 ms per run)
whisper_print_timings:   batchd time =  1642.23 ms /    36 runs (    45.62 ms per run)
whisper_print_timings:   prompt time =   974.67 ms /    44 runs (    22.15 ms per run)
whisper_print_timings:    total time = 12370.97 ms";
        // 5183.09 + 75.94 + 1642.23 + 974.67, with load and total left out.
        assert_eq!(compute_ms(stderr), Some(7876));
    }

    #[test]
    fn output_without_timings_is_not_a_zero_millisecond_result() {
        // A backend that failed to start prints an error, not timings. Scoring that
        // as instant would make the broken one win.
        assert_eq!(compute_ms("error: failed to initialize Vulkan device"), None);
        assert_eq!(compute_ms(""), None);
    }

    fn t(backend: Backend, threads: usize, ms: u64) -> Trial {
        Trial { backend, threads, ms }
    }

    #[test]
    fn the_fastest_trial_wins() {
        let m = Measured {
            trials: vec![
                t(Backend::Cpu, 8, 7876),
                t(Backend::Vulkan, 8, 2100),
                t(Backend::Vulkan, 4, 2050),
            ],
            real_audio: true,
        };
        assert_eq!(m.best(), Some(t(Backend::Vulkan, 4, 2050)));
        assert_eq!(Measured { trials: vec![], real_audio: false }.best(), None);
    }

    #[test]
    fn each_backend_is_summarised_by_its_best_run() {
        // The settings screen shows one number per processor, and it should be that
        // processor at its best rather than at whatever thread count came last.
        let m = Measured {
            trials: vec![
                t(Backend::Cpu, 8, 7876),
                t(Backend::Cpu, 12, 6200),
                t(Backend::Vulkan, 8, 2100),
            ],
            real_audio: true,
        };
        let by = m.by_backend();
        assert_eq!(by.get("cpu"), Some(&6200));
        assert_eq!(by.get("vulkan"), Some(&2100));
        assert_eq!(by.get("cuda"), None);
    }
}
