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
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One thing that happened during a recorded service — the raw material the review step
/// and learning use. The trust tier is derived from `kind`:
///   * "confirmed" — auto-projected AND the speaker read the verse aloud (Gold)
///   * "operator"  — the operator projected/picked it themselves (Silver)
///   * "auto"      — auto-projected, no confirmation yet (Bronze)
///   * "corrected" — auto-projected, then the operator swapped it out (Negative: a
///                   labelled mistake — what was heard -> the wrong guess -> the right one)
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Moment {
    pub kind: String,
    pub reference: String,
    pub book_osis: String,
    pub chapter: u16,
    pub verse: u16,
    /// The recogniser's confidence, when the app chose it.
    #[serde(default)]
    pub confidence: f32,
    /// How it was recognised ("explicit", "fuzzy", "quote", …).
    #[serde(default)]
    pub source: String,
    /// What the speaker was heard to say around this moment.
    #[serde(default)]
    pub spoken: String,
    /// For a correction: the wrong reference the operator replaced.
    #[serde(default)]
    pub replaced: String,
}

/// The sidecar written beside a recorded service's audio: what happened during it, and
/// whether the operator has since looked at it. Nothing is ever learned from a service
/// the operator has not approved — review, not silent auto-learning.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Labels {
    #[serde(default)]
    pub moments: Vec<Moment>,
    #[serde(default)]
    pub saved_at: String,
    #[serde(default)]
    pub approved: bool,
}

/// One recorded service as the end-of-service review sees it.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    /// The timestamp name, which is also how the review addresses this service.
    pub name: String,
    pub saved_at: String,
    pub minutes: f32,
    pub approved: bool,
    pub moments: Vec<Moment>,
}

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

/// The labels beside a recorded service. A missing or unreadable sidecar reads as an
/// empty, unapproved service rather than an error: the audio is still the useful part.
pub fn read_labels(audio: &Path) -> Labels {
    std::fs::read_to_string(audio.with_extension("json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn write_labels(audio: &Path, labels: &Labels) -> std::io::Result<()> {
    let json = serde_json::to_string(labels)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(audio.with_extension("json"), json)
}

/// How long a recorded service runs, from the WAV header.
pub fn minutes(audio: &Path) -> f32 {
    hound::WavReader::open(audio)
        .map(|r| {
            let rate = r.spec().sample_rate.max(1) as f32;
            r.duration() as f32 / rate / 60.0
        })
        .unwrap_or(0.0)
}

/// Delete one recorded service — audio and labels together, so no orphan sidecar is
/// left behind claiming a service that no longer exists.
pub fn discard(audio: &Path) {
    let _ = std::fs::remove_file(audio.with_extension("json"));
    let _ = std::fs::remove_file(audio);
}

/// Every recorded service for one speaker, newest first.
pub fn summaries(dir: &Path) -> Vec<SessionSummary> {
    list_audio(dir)
        .into_iter()
        .map(|audio| {
            let labels = read_labels(&audio);
            SessionSummary {
                name: audio.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                saved_at: labels.saved_at,
                minutes: minutes(&audio),
                approved: labels.approved,
                moments: labels.moments,
            }
        })
        .collect()
}

/// The audio for one named service in `dir`. `None` when the name isn't a plain service
/// name — a name arriving from the UI must never be able to reach outside this folder.
pub fn audio_named(dir: &Path, name: &str) -> Option<PathBuf> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let audio = dir.join(format!("{name}.wav"));
    audio.exists().then_some(audio)
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

/// Where a live recording is written: the speaker's folder, the service's name, and how
/// many services to keep afterwards.
#[derive(Clone, Debug)]
pub struct RecordTarget {
    pub dir: PathBuf,
    pub name: String,
    pub keep: usize,
}

/// Streams the live 16 kHz mono feed straight to a WAV as the service happens, so a whole
/// sermon never has to sit in memory. On `finish` it writes the labels sidecar and trims
/// the speaker's folder to the rolling window.
pub struct Recorder {
    writer: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>,
    target: RecordTarget,
    written: usize,
}

/// Below this much audio a recording is not a service — a mis-click, a soundcheck — and
/// keeping it would only push a real sermon out of the rolling window.
const MIN_RECORDING_SAMPLES: usize = 16_000 * 20; // 20 seconds

impl Recorder {
    pub fn start(target: RecordTarget) -> std::io::Result<Recorder> {
        std::fs::create_dir_all(&target.dir)?;
        let path = target.dir.join(format!("{}.wav", target.name));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let writer = hound::WavWriter::create(&path, spec).map_err(io_err)?;
        Ok(Recorder { writer: Some(writer), target, written: 0 })
    }

    /// Append one captured frame (16 kHz mono float). Errors are swallowed: a dropped
    /// sample must never take down the live listening loop.
    pub fn push(&mut self, frame: &[f32]) {
        if let Some(w) = self.writer.as_mut() {
            for &s in frame {
                let _ = w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
            }
            self.written += frame.len();
        }
    }

    /// Finalize the audio. If it is long enough to be a real service, save the labels
    /// sidecar and enforce the rolling window; otherwise discard it so a mis-click can't
    /// evict a real sermon. Returns the saved audio path, if kept.
    pub fn finish(mut self, labels_json: &str) -> Option<PathBuf> {
        if let Some(w) = self.writer.take() {
            let _ = w.finalize();
        }
        let audio = self.target.dir.join(format!("{}.wav", self.target.name));
        if self.written < MIN_RECORDING_SAMPLES {
            let _ = std::fs::remove_file(&audio);
            return None;
        }
        let _ = std::fs::write(self.target.dir.join(format!("{}.json", self.target.name)), labels_json);
        enforce_window(&self.target.dir, self.target.keep);
        Some(audio)
    }
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

    fn moment(kind: &str, reference: &str) -> Moment {
        Moment {
            kind: kind.into(),
            reference: reference.into(),
            book_osis: "John".into(),
            chapter: 3,
            verse: 16,
            confidence: 0.9,
            source: "explicit".into(),
            spoken: "john chapter three verse sixteen".into(),
            replaced: String::new(),
        }
    }

    #[test]
    fn a_service_is_unapproved_until_the_operator_says_otherwise() {
        let base = std::env::temp_dir().join(format!("nb-review-{}", std::process::id()));
        let dir = dir_for(&base, "President");
        let samples = vec![0.0f32; 1600];
        let labels = Labels {
            moments: vec![moment("auto", "John 3:16"), moment("corrected", "Rom 8:28")],
            saved_at: now_stamp(),
            approved: false,
        };
        let audio =
            save_session(&dir, "0000000000001", &samples, &serde_json::to_string(&labels).unwrap(), MAX_KEEP)
                .unwrap();

        let seen = summaries(&dir);
        assert_eq!(seen.len(), 1);
        assert!(!seen[0].approved, "a fresh recording must not count as reviewed");
        assert_eq!(seen[0].moments.len(), 2);
        assert_eq!(seen[0].name, "0000000000001");

        // Approving keeps only what the operator left in place.
        let mut kept = read_labels(&audio);
        kept.moments.truncate(1);
        kept.approved = true;
        write_labels(&audio, &kept).unwrap();

        let seen = summaries(&dir);
        assert!(seen[0].approved);
        assert_eq!(seen[0].moments.len(), 1);

        discard(&audio);
        assert!(!audio.exists() && !audio.with_extension("json").exists());
        assert!(summaries(&dir).is_empty());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_missing_sidecar_reads_as_an_empty_unapproved_service() {
        let dir = std::env::temp_dir().join(format!("nb-bare-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("0000000000002.wav");
        std::fs::write(&audio, b"not really audio").unwrap();
        let labels = read_labels(&audio);
        assert!(!labels.approved);
        assert!(labels.moments.is_empty());
        assert_eq!(minutes(&audio), 0.0, "an unreadable WAV must not report a length");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_service_name_cannot_reach_outside_the_speakers_folder() {
        let dir = std::env::temp_dir().join(format!("nb-name-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("0000000000003.wav"), b"a").unwrap();

        assert!(audio_named(&dir, "0000000000003").is_some());
        assert!(audio_named(&dir, "0000000000004").is_none(), "no such service");
        for bad in ["..\\..\\secrets", "../../secrets", "a/b", "", "with space", "dot.dot"] {
            assert!(audio_named(&dir, bad).is_none(), "{bad} must be rejected");
        }
        std::fs::remove_dir_all(&dir).ok();
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
