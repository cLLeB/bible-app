//! A rolling, per-profile store of recorded services — on this machine only.
//!
//! Each recorded service is one audio file (16 kHz mono, what the recognizer uses) plus a
//! `.json` sidecar of the moments captured during it (what was projected, corrected, or
//! confirmed). To bound disk use we keep at most `keep` most-recent services per speaker —
//! default 5, never fewer than 2 — and delete the rest, oldest first, every time.
//!
//! Files are named by an ISO-ish timestamp so a lexical sort is a chronological sort:
//! `2026-07-16T15-30-05.wav` / `.json`.

use crate::db::Db;
use std::path::{Path, PathBuf};

/// Most services we keep per speaker. More would waste the church's disk for little gain.
pub const MAX_KEEP: usize = 5;
/// Fewer than this and there is not enough to tune a voice on.
pub const MIN_KEEP: usize = 2;

/// Clamp an operator-chosen window to the allowed range (default to the max).
pub fn clamp_keep(n: usize) -> usize {
    n.clamp(MIN_KEEP, MAX_KEEP)
}

/// A filesystem-safe folder name for a speaker, so "Vice-President" or a typed guest name
/// never produces an unusable path.
pub fn slug(profile: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in profile.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-'); // collapse any run of separators to a single dash
            prev_dash = true;
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() { "speaker".into() } else { s }
}

/// The recorded-service audio files in `dir`, newest first.
pub fn list_audio(dir: &Path) -> Vec<PathBuf> {
    let mut audio: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("wav"))
            .collect(),
        Err(_) => Vec::new(),
    };
    // Filenames are timestamps, so a reverse lexical sort is newest-first.
    audio.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    audio
}

/// Keep only the newest `keep` services in `dir`; delete the rest (audio + its `.json`
/// sidecar). Returns the paths removed. `keep` is clamped to the allowed window.
pub fn enforce_window(dir: &Path, keep: usize) -> Vec<PathBuf> {
    let keep = clamp_keep(keep);
    let mut removed = Vec::new();
    for audio in list_audio(dir).into_iter().skip(keep) {
        let sidecar = audio.with_extension("json");
        let _ = std::fs::remove_file(&sidecar);
        if std::fs::remove_file(&audio).is_ok() {
            removed.push(audio);
        }
    }
    removed
}

/// The per-speaker session folder under the app's data directory.
pub fn dir_for(base: &Path, profile: &str) -> PathBuf {
    base.join("sessions").join(slug(profile))
}

/// A sortable, unique name for a new service — epoch milliseconds, zero-padded so a
/// lexical sort stays chronological (good until well past year 2200).
pub fn now_stamp() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{ms:013}")
}

fn io_err(e: hound::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, e)
}

/// Write the recognizer's own 16 kHz mono float samples out as a small WAV.
fn write_wav_16k(path: &Path, samples: &[f32]) -> std::io::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(io_err)?;
    for &s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).map_err(io_err)?;
    }
    w.finalize().map_err(io_err)
}

/// Save one recorded service — its 16 kHz mono audio plus a JSON sidecar of what happened
/// — into the speaker's folder under `name`, then trim to the newest `keep`. Returns the
/// audio path. Everything stays on this machine.
pub fn save_session(
    dir: &Path,
    name: &str,
    samples: &[f32],
    labels_json: &str,
    keep: usize,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let audio = dir.join(format!("{name}.wav"));
    write_wav_16k(&audio, samples)?;
    std::fs::write(dir.join(format!("{name}.json")), labels_json)?;
    enforce_window(dir, keep);
    Ok(audio)
}

/// How many services the operator keeps per speaker — clamped to [2, 5], default 5.
pub fn window_size(db: &Db) -> usize {
    db.get_setting("session_window")
        .and_then(|v| v.parse::<usize>().ok())
        .map(clamp_keep)
        .unwrap_or(MAX_KEEP)
}

pub fn set_window_size(db: &Db, n: usize) -> rusqlite::Result<()> {
    db.set_setting("session_window", &clamp_keep(n).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_filesystem_safe() {
        assert_eq!(slug("Vice-President"), "vice-president");
        assert_eq!(slug("Guest — Pastor Mensah"), "guest-pastor-mensah");
        assert_eq!(slug("   "), "speaker");
    }

    #[test]
    fn window_is_clamped_between_two_and_five() {
        assert_eq!(clamp_keep(0), 2);
        assert_eq!(clamp_keep(1), 2);
        assert_eq!(clamp_keep(3), 3);
        assert_eq!(clamp_keep(9), 5);
    }

    #[test]
    fn only_the_newest_are_kept_oldest_deleted_with_their_sidecars() {
        let dir = std::env::temp_dir().join(format!("nb-sessions-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Six services, timestamped so lexical order = chronological order.
        let stamps = [
            "2026-07-01T10-00-00",
            "2026-07-02T10-00-00",
            "2026-07-03T10-00-00",
            "2026-07-04T10-00-00",
            "2026-07-05T10-00-00",
            "2026-07-06T10-00-00",
        ];
        for s in stamps {
            std::fs::write(dir.join(format!("{s}.wav")), b"a").unwrap();
            std::fs::write(dir.join(format!("{s}.json")), b"{}").unwrap();
        }

        let removed = enforce_window(&dir, MAX_KEEP);
        assert_eq!(removed.len(), 1, "the single oldest should be removed");

        let kept = list_audio(&dir);
        assert_eq!(kept.len(), MAX_KEEP);
        // Newest first, and the oldest (July 1) is gone along with its sidecar.
        assert!(kept[0].file_name().unwrap().to_string_lossy().starts_with("2026-07-06"));
        assert!(!dir.join("2026-07-01T10-00-00.wav").exists());
        assert!(!dir.join("2026-07-01T10-00-00.json").exists());
        assert!(dir.join("2026-07-02T10-00-00.json").exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dir_is_under_sessions_by_speaker_slug() {
        let d = dir_for(Path::new("/data"), "Vice-President");
        assert!(d.ends_with("sessions/vice-president") || d.ends_with("sessions\\vice-president"));
    }

    #[test]
    fn saving_writes_audio_plus_labels_and_trims_the_window() {
        let base = std::env::temp_dir().join(format!("nb-save-{}", std::process::id()));
        let dir = dir_for(&base, "President");
        let samples = vec![0.0f32; 1600]; // 0.1s of silence at 16 kHz
        for i in 0..(MAX_KEEP + 1) {
            let name = format!("{:013}", 1_000_000 + i); // sortable stamps
            let audio = save_session(&dir, &name, &samples, "{\"moments\":[]}", MAX_KEEP).unwrap();
            assert!(audio.exists());
            assert!(audio.with_extension("json").exists());
        }
        // The extra one over the window was trimmed, audio + sidecar together.
        assert_eq!(list_audio(&dir).len(), MAX_KEEP);
        assert!(!dir.join("0001000000.wav").exists()); // wrong width, never existed
        assert!(!dir.join(format!("{:013}.json", 1_000_000)).exists()); // oldest sidecar gone
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn window_size_defaults_to_five_and_clamps() {
        let db = crate::db::open_at(Path::new(":memory:")).unwrap();
        db.migrate().unwrap();
        assert_eq!(window_size(&db), MAX_KEEP);
        set_window_size(&db, 3).unwrap();
        assert_eq!(window_size(&db), 3);
        set_window_size(&db, 99).unwrap(); // clamped down to 5
        assert_eq!(window_size(&db), MAX_KEEP);
        set_window_size(&db, 1).unwrap(); // clamped up to 2
        assert_eq!(window_size(&db), MIN_KEEP);
    }
}
