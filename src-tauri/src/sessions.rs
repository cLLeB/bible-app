//! A rolling, per-profile store of recorded services — on this machine only.
//!
//! Each recorded service is one audio file (16 kHz mono, what the recognizer uses) plus a
//! `.json` sidecar of the moments captured during it (what was projected, corrected, or
//! confirmed). To bound disk use we keep at most `keep` most-recent services per speaker —
//! default 5, never fewer than 2 — and delete the rest, oldest first, every time.
//!
//! Files are named by an ISO-ish timestamp so a lexical sort is a chronological sort:
//! `2026-07-16T15-30-05.wav` / `.json`.

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
}
