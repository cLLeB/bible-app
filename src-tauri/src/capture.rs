//! Utterance capture for voice calibration.
//!
//! When enabled, every endpointed utterance is written to `<app-data>/captures`
//! as a 16 kHz mono WAV alongside the model's raw transcript. That gives a real
//! corpus — this speaker, this mic, this room — to tune decode settings against,
//! instead of guessing from a synthetic benchmark.
//!
//! Enabled by the `BIBLE_APP_CAPTURE` env var (dev/bench runs) or at runtime by
//! the in-app calibration wizard. Captures never leave the machine.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager};

/// Runtime switch, flipped by the calibration wizard. The env var forces it on
/// for bench runs without any UI.
static RUNTIME: AtomicBool = AtomicBool::new(false);

#[allow(dead_code)] // used by the calibration wizard, still to be wired up
pub fn set_enabled(on: bool) {
    RUNTIME.store(on, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    RUNTIME.load(Ordering::Relaxed)
        || std::env::var("BIBLE_APP_CAPTURE").map(|v| v != "0" && !v.is_empty()).unwrap_or(false)
}

pub fn dir(app: &AppHandle) -> Option<PathBuf> {
    let d = app.path().app_data_dir().ok()?.join("captures");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

/// Next free utterance index, so repeated sessions append rather than overwrite.
fn next_index(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    e.path().extension().map(|x| x.eq_ignore_ascii_case("wav")).unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

/// Persist one utterance: the audio the model heard, and what it made of it.
pub fn save(app: &AppHandle, samples16k: &[f32], raw: &str, cleaned: &str, model: &Path) {
    if !enabled() {
        return;
    }
    let Some(dir) = dir(app) else { return };
    let n = next_index(&dir);
    let wav = dir.join(format!("utt_{n:03}.wav"));
    if let Err(e) = crate::stt::write_wav_16k_mono(&wav, samples16k) {
        eprintln!("capture: could not write {}: {e}", wav.display());
        return;
    }
    let entry = serde_json::json!({
        "n": n,
        "wav": wav.file_name().map(|f| f.to_string_lossy().to_string()),
        "model": model.file_name().map(|f| f.to_string_lossy().to_string()),
        "raw": raw,
        "cleaned": cleaned,
        "seconds": samples16k.len() as f32 / 16_000.0,
    });
    let line = format!("{entry}\n");
    let manifest = dir.join("manifest.jsonl");
    if let Err(e) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&manifest)
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()))
    {
        eprintln!("capture: could not append manifest: {e}");
    }
}
