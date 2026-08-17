//! Measuring this machine, rather than assuming things about laptops in general.
//!
//! The ranking in `accel` says a graphics card beats a processor, which is true of
//! most hardware and false of some. Integrated Intel graphics in a 15W laptop is the
//! awkward case: a Vulkan pass there can lose outright to eight CPU threads, and the
//! machines where that happens are exactly the ones already short of speed. The only
//! way to know is to run it.
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

/// Speech-shaped audio to time against, used only when no real utterance has been
/// captured from this room.
///
/// Synthetic audio understates decode cost, because the model finds little to say
/// about it, and decode is the part a graphics card helps least with. So a probe run
/// on this clip flatters the GPU slightly. A real captured utterance is preferred
/// wherever one exists, for exactly that reason.
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

/// The newest real utterance this machine has captured, if any. Preferred over
/// synthetic audio: it exercises the decoder the way a sermon does.
fn captured_clip(captures: Option<&Path>) -> Option<Vec<f32>> {
    let dir = captures?;
    let mut wavs: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x.eq_ignore_ascii_case("wav")).unwrap_or(false))
        .collect();
    wavs.sort();
    let path = wavs.last()?;
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

/// Run one trial. Returns None if that backend cannot run here at all, which is a
/// perfectly ordinary outcome: a build may ship a CUDA backend to a machine with an
/// AMD card in it.
fn trial(bin_dir: &Path, model: &Path, wav: &Path, backend: Backend, threads: usize) -> Option<Trial> {
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
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().ok()?;
    let ms = compute_ms(&String::from_utf8_lossy(&out.stderr))?;
    Some(Trial { backend, threads, ms })
}

/// Time every backend this machine can run, then sweep the thread count on the
/// winner. Sweeping threads on all of them would multiply the trials for an answer
/// that only matters on whichever one is actually going to be used.
///
/// `report` is called after each trial so a long measurement can show progress
/// rather than appearing to hang.
pub fn measure(
    bin_root: &Path,
    model: &Path,
    captures: Option<&Path>,
    mut report: impl FnMut(&Trial),
) -> Result<Vec<Trial>, String> {
    let backends = crate::accel::available(bin_root);
    if backends.is_empty() {
        return Err("No whisper build was found to measure.".into());
    }
    let clip = captured_clip(captures).unwrap_or_else(synthetic_clip);
    let wav = std::env::temp_dir().join(format!("bibleapp_probe_{}.wav", std::process::id()));
    crate::stt::write_wav_16k_mono(&wav, &clip)?;

    let base_threads: usize = crate::stt::threads().parse().unwrap_or(4);
    let mut results: Vec<Trial> = Vec::new();

    for b in &backends {
        let Some(dir) = crate::accel::dir_for(bin_root, *b) else { continue };
        if let Some(t) = trial(&dir, model, &wav, *b, base_threads) {
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
                if let Some(t) = trial(&dir, model, &wav, best.backend, n) {
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
    Ok(results)
}

/// The winning trial: fewest milliseconds per utterance.
pub fn best(trials: &[Trial]) -> Option<Trial> {
    trials.iter().min_by_key(|t| t.ms).copied()
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

    #[test]
    fn the_fastest_trial_wins() {
        let t = |b, threads, ms| Trial { backend: b, threads, ms };
        let trials =
            [t(Backend::Cpu, 8, 7876), t(Backend::Vulkan, 8, 2100), t(Backend::Vulkan, 4, 2050)];
        assert_eq!(best(&trials), Some(t(Backend::Vulkan, 4, 2050)));
        assert_eq!(best(&[]), None);
    }
}
