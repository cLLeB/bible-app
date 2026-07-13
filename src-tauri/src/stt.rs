//! Speech-to-text against a resident whisper model.
//!
//! The model is loaded once and kept in memory for the life of the process.
//! The previous design shelled out to `whisper-cli.exe` per utterance, which
//! re-read the model from disk every time (148 MB for base, 488 MB for small) —
//! so latency was dominated by loading, not decoding, and the cheap models got
//! no faster than the expensive ones. With the model resident there is enough
//! headroom to decode with beam search, which is where accuracy lives.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

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

/// How to decode. Held apart from the model so the bench can sweep settings and
/// the calibration wizard can persist a per-speaker choice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decode {
    /// 1 = greedy. Higher searches more candidate transcripts — slower, more accurate.
    pub beam: i32,
    /// Prime the decoder with scripture vocabulary. Helps a strong model; can
    /// drag a weak one around, so it is a knob rather than a constant.
    pub prompt: bool,
    /// Scale speech to a consistent level before decoding. A quiet mic hurts
    /// the small models most.
    pub normalize: bool,
    /// Shrink the encoder window to fit the clip. whisper always encodes a fixed
    /// 30-second window, so a 3-second reference pays for 27 seconds of silence —
    /// and encode dominates the cost (~3.7s vs ~70ms of decode on base here).
    /// Sized from the clip with headroom; false = whisper's full 1500 frames.
    pub fit_window: bool,
}

impl Default for Decode {
    fn default() -> Self {
        Self { beam: 5, prompt: true, normalize: true, fit_window: true }
    }
}

/// Encoder frames to use for `secs` of audio. whisper's encoder runs over 1500
/// frames = 30s, so ~50 frames per second of speech; the margin covers whisper's
/// need for some context past the end of the utterance.
fn audio_ctx_for(secs: f32) -> std::os::raw::c_int {
    const FRAMES_PER_SEC: f32 = 50.0;
    const FULL: f32 = 1500.0;
    let wanted = (secs * FRAMES_PER_SEC * 1.5).ceil().clamp(256.0, FULL);
    wanted as std::os::raw::c_int
}

impl Decode {
    /// Bench/dev override, e.g. `BIBLE_APP_DECODE=beam=1,prompt=0,normalize=1`.
    pub fn from_env_or_default() -> Self {
        let mut d = Self::default();
        let Ok(spec) = std::env::var("BIBLE_APP_DECODE") else { return d };
        for part in spec.split(',') {
            let Some((k, v)) = part.split_once('=') else { continue };
            match k.trim() {
                "beam" => d.beam = v.trim().parse().unwrap_or(d.beam),
                "prompt" => d.prompt = v.trim() != "0",
                "normalize" => d.normalize = v.trim() != "0",
                "fit_window" => d.fit_window = v.trim() != "0",
                _ => {}
            }
        }
        d
    }
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

/// The loaded model. Keyed by path so switching model tier reloads, and only then.
static MODEL: Mutex<Option<(PathBuf, WhisperContext)>> = Mutex::new(None);

fn threads() -> i32 {
    std::thread::available_parallelism().map(|n| n.get().min(8)).unwrap_or(4) as i32
}

/// Load `model` if it isn't already resident, then run `f` against it.
fn with_model<T>(
    model: &Path,
    f: impl FnOnce(&WhisperContext) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = MODEL.lock().map_err(|e| e.to_string())?;
    let stale = guard.as_ref().map(|(p, _)| p != model).unwrap_or(true);
    if stale {
        let path = model.to_str().ok_or("bad model path")?;
        let ctx = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .map_err(|e| format!("could not load whisper model {}: {e}", model.display()))?;
        *guard = Some((model.to_path_buf(), ctx));
    }
    let (_, ctx) = guard.as_ref().expect("model just loaded");
    f(ctx)
}

/// Load the model ahead of first speech, so the first utterance isn't slowed by
/// a cold load. Safe to call repeatedly.
pub fn preload(model: &Path) -> Result<(), String> {
    with_model(model, |_| Ok(()))
}

/// Transcribe 16 kHz mono f32 samples.
pub fn transcribe(samples16k: &[f32], model: &Path, decode: Decode) -> Result<String, String> {
    let audio =
        if decode.normalize { normalize(samples16k) } else { samples16k.to_vec() };

    with_model(model, |ctx| {
        let mut state = ctx.create_state().map_err(|e| e.to_string())?;

        let strategy = if decode.beam > 1 {
            SamplingStrategy::BeamSearch { beam_size: decode.beam, patience: -1.0 }
        } else {
            SamplingStrategy::Greedy { best_of: 1 }
        };
        let mut params = FullParams::new(strategy);
        params.set_n_threads(threads());
        params.set_language(Some("en"));
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_no_context(true);
        if decode.fit_window {
            params.set_audio_ctx(audio_ctx_for(audio.len() as f32 / 16_000.0));
        }
        if decode.prompt {
            params.set_initial_prompt(BIBLE_PROMPT);
        }

        state.full(params, &audio).map_err(|e| format!("whisper failed: {e}"))?;

        let n = state.full_n_segments().map_err(|e| e.to_string())?;
        let mut text = String::new();
        for i in 0..n {
            if let Ok(seg) = state.full_get_segment_text(i) {
                text.push_str(&seg);
                text.push(' ');
            }
        }
        Ok(text.trim().to_string())
    })
}
