//! Speech-to-text via the bundled whisper.cpp binary.
//!
//! The binary ships with per-CPU backend DLLs and picks an optimized one at
//! runtime (AVX2/FMA on anything modern). A statically linked build was tried and
//! decoded ~6x slower — those runtime-dispatched kernels are the whole game — so
//! the process spawn stays, and the cost that actually mattered gets fixed here
//! instead: the encoder window.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Windows spawns a console window for a console-subsystem binary unless told
/// not to. whisper runs once per interim pass and once per endpoint, so without
/// this a black window flashes several times per spoken sentence.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

static COUNTER: AtomicU64 = AtomicU64::new(0);

// Bias whisper toward scripture vocabulary. whisper only consumes the last
// ~224 prompt tokens and weights later tokens most, so the hard-to-hear rare
// names (books AND famous people/places) go LAST for maximum influence.
const BIBLE_PROMPT: &str = "A spoken Bible scripture reference, for example John chapter 3 verse 16 or Romans 8:28. \
Books: Genesis, Exodus, Leviticus, Numbers, Deuteronomy, Joshua, Judges, Ruth, 1 Samuel, 2 Samuel, 1 Kings, 2 Kings, \
1 Chronicles, 2 Chronicles, Ezra, Esther, Job, Psalms, Proverbs, Isaiah, Jeremiah, Ezekiel, Daniel, Hosea, Joel, Amos, \
Jonah, Matthew, Mark, Luke, John, Acts, Romans, 1 Corinthians, 2 Corinthians, Galatians, Titus, Hebrews, James, Jude, \
Revelation, chapter and verse. Also these harder names: Nehemiah, Ecclesiastes, Song of Solomon, Lamentations, Obadiah, \
Micah, Nahum, Habakkuk, Zephaniah, Haggai, Zechariah, Malachi, Ephesians, Philippians, Colossians, 1 Thessalonians, \
2 Thessalonians, 1 Timothy, 2 Timothy, Philemon, 1 Peter, 2 Peter; and Nebuchadnezzar, Melchizedek, Zacchaeus, \
Methuselah, Mephibosheth, Habakkuk, Nicodemus, Zerubbabel, Bartimaeus, Gethsemane, Zacchaeus, Philippians.";

/// How to decode. A struct rather than constants so the bench can sweep it and
/// the calibration wizard can persist a per-speaker choice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decode {
    /// 1 = greedy. Higher searches more candidate transcripts. Measured cost on
    /// base with a fitted window: ~66ms. Effectively free, so it defaults on.
    pub beam: i32,
    /// Prime the decoder with scripture vocabulary.
    pub prompt: bool,
    /// Scale speech to a consistent level before decoding — a quiet mic hurts
    /// the smaller models most.
    pub normalize: bool,
    /// Encoder frames. whisper always encodes a 30s window (1500 frames) unless
    /// told otherwise, so a 3s reference pays to encode 27s of silence: 2489ms
    /// vs 392ms measured on base. `Fit` sizes the window to the clip.
    pub window: Window,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Window {
    /// whisper's default: the full 30 seconds.
    Full,
    /// Sized to the clip, with `frames_per_sec * margin` headroom. Too tight and
    /// the decoder can loop on itself and repeat the phrase; too loose and the
    /// encode cost comes back.
    Fit { margin: f32 },
}

impl Default for Decode {
    fn default() -> Self {
        Self { beam: 5, prompt: true, normalize: true, window: Window::Fit { margin: 1.5 } }
    }
}

impl Decode {
    /// Bench/dev override, e.g. `BIBLE_APP_DECODE=beam=1,prompt=0,window=full`.
    pub fn from_env_or_default() -> Self {
        let mut d = Self::default();
        let Ok(spec) = std::env::var("BIBLE_APP_DECODE") else { return d };
        for part in spec.split(',') {
            let Some((k, v)) = part.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "beam" => d.beam = v.parse().unwrap_or(d.beam),
                "prompt" => d.prompt = v != "0",
                "normalize" => d.normalize = v != "0",
                "window" => {
                    d.window = if v == "full" {
                        Window::Full
                    } else {
                        Window::Fit { margin: v.parse().unwrap_or(1.5) }
                    }
                }
                _ => {}
            }
        }
        d
    }

    /// Encoder frames for `secs` of audio: whisper's encoder covers 30s in 1500
    /// frames, so 50 frames per second of speech, plus margin.
    fn audio_ctx(&self, secs: f32) -> Option<i32> {
        const FRAMES_PER_SEC: f32 = 50.0;
        const FULL: f32 = 1500.0;
        match self.window {
            Window::Full => None,
            Window::Fit { margin } => {
                Some((secs * FRAMES_PER_SEC * margin).ceil().clamp(192.0, FULL) as i32)
            }
        }
    }
}

fn temp_base() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("bibleapp_utt_{ts}_{n}"))
}

/// Peak-normalize toward a consistent loudness, leaving headroom. Silence and
/// already-loud audio are left alone.
fn normalize(samples: &[f32]) -> Vec<f32> {
    const TARGET_PEAK: f32 = 0.85;
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if peak < 1e-4 || peak >= TARGET_PEAK {
        return samples.to_vec();
    }
    let gain = (TARGET_PEAK / peak).min(8.0);
    samples.iter().map(|s| (s * gain).clamp(-1.0, 1.0)).collect()
}

pub fn write_wav_16k_mono(path: &Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        writer
            .write_sample((clamped * i16::MAX as f32) as i16)
            .map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

/// Transcribe 16 kHz mono f32 samples by invoking the whisper.cpp binary.
pub fn transcribe(
    samples16k: &[f32],
    model: &Path,
    binary: &Path,
    decode: Decode,
) -> Result<String, String> {
    let audio = if decode.normalize { normalize(samples16k) } else { samples16k.to_vec() };
    let base = temp_base();
    let wav_path = base.with_extension("wav");
    write_wav_16k_mono(&wav_path, &audio)?;

    let threads = std::thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(4)
        .to_string();
    let beam = decode.beam.to_string();
    let secs = audio.len() as f32 / 16_000.0;

    let mut cmd = Command::new(binary);
    cmd.args([
        "-m",
        model.to_str().ok_or("bad model path")?,
        "-f",
        wav_path.to_str().ok_or("bad wav path")?,
        "-l",
        "en",
        "-t",
        &threads,
        "-bs",
        &beam,
        "-bo",
        &beam,
        "-nt",
        "-otxt",
        "-of",
        base.to_str().ok_or("bad out path")?,
    ]);
    if let Some(ctx) = decode.audio_ctx(secs) {
        cmd.args(["-ac", &ctx.to_string()]);
    }
    if decode.prompt {
        cmd.args(["--prompt", BIBLE_PROMPT]);
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd.output().map_err(|e| format!("failed to run whisper binary: {e}"))?;

    let txt_path = base.with_extension("txt");
    let result = std::fs::read_to_string(&txt_path).map(|t| t.trim().to_string());

    let _ = std::fs::remove_file(&wav_path);
    let _ = std::fs::remove_file(&txt_path);

    match result {
        Ok(text) => Ok(dedupe_repeats(&text)),
        Err(_) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Err(format!("whisper produced no output: {stderr}"))
        }
    }
}

/// A tight encoder window can make the decoder loop, emitting the same phrase
/// twice ("John chapter 3 verse 16 John chapter 3 verse 16"). Harmless to the
/// reference parser, but it pollutes the transcript and the quote matcher, so
/// collapse an immediately repeated tail.
fn dedupe_repeats(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let n = words.len();
    if n < 4 {
        return text.trim().to_string();
    }
    // Longest repeated suffix: if the last half equals the half before it, drop it.
    for len in (2..=n / 2).rev() {
        let (a, b) = (&words[n - 2 * len..n - len], &words[n - len..]);
        let same = a
            .iter()
            .zip(b.iter())
            .all(|(x, y)| x.trim_matches(|c: char| !c.is_alphanumeric()).eq_ignore_ascii_case(
                y.trim_matches(|c: char| !c.is_alphanumeric()),
            ));
        if same {
            return words[..n - len].join(" ");
        }
    }
    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_a_repeated_phrase() {
        assert_eq!(
            dedupe_repeats("John chapter 3 verse 16 John chapter 3 verse 16"),
            "John chapter 3 verse 16"
        );
    }

    #[test]
    fn leaves_normal_speech_alone() {
        let s = "Turn with me to Habakkuk chapter 2 verse 4";
        assert_eq!(dedupe_repeats(s), s);
    }

    #[test]
    fn window_fits_the_clip_and_has_a_floor() {
        let d = Decode::default();
        assert_eq!(d.audio_ctx(3.0), Some(225));
        assert_eq!(d.audio_ctx(0.5), Some(192)); // floor
        assert_eq!(d.audio_ctx(60.0), Some(1500)); // never exceeds whisper's window
        assert_eq!(Decode { window: Window::Full, ..d }.audio_ctx(3.0), None);
    }
}
