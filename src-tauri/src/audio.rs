use crate::commands::{build_payload, AppState};
use crate::detect::{self, DetectSource, Detection, RefContext};
use crate::events::Candidate;
use crate::{semantic, stt};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const TARGET_RATE: u32 = 16_000;
const SILENCE_RMS: f32 = 0.010;
const SILENCE_FLUSH_MS: f32 = 1300.0;
const MAX_UTTER_MS: f32 = 12_000.0;
const MIN_SPEECH_MS: f32 = 600.0;
const PREROLL_SAMPLES: usize = (TARGET_RATE as usize) * 3 / 10; // 0.3s pre-speech
const CTX_STALE_SECS: u64 = 300; // clear remembered book after 5min of no speech
const INTERIM_SAMPLES: usize = (TARGET_RATE as usize) * 4; // interim pass every 4s of long speech

type RefKey = (String, u16, Option<u16>);

fn rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum: f32 = frame.iter().map(|s| s * s).sum();
    (sum / frame.len() as f32).sqrt()
}

fn ms_of(samples: usize) -> f32 {
    samples as f32 / TARGET_RATE as f32 * 1000.0
}

fn downmix_resample(data: &[f32], channels: usize, src_rate: f32) -> Vec<f32> {
    let mono: Vec<f32> = if channels <= 1 {
        data.to_vec()
    } else {
        data.chunks(channels)
            .map(|c| c.iter().sum::<f32>() / channels as f32)
            .collect()
    };
    if (src_rate - TARGET_RATE as f32).abs() < 1.0 {
        return mono;
    }
    let ratio = TARGET_RATE as f32 / src_rate;
    let out_len = (mono.len() as f32 * ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_idx = (i as f32 / ratio) as usize;
        if src_idx < mono.len() {
            out.push(mono[src_idx]);
        }
    }
    out
}

fn clean_transcript(raw: &str) -> String {
    let joined = raw
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| !(l.starts_with('[') && l.ends_with(']')))
        .filter(|l| !(l.starts_with('(') && l.ends_with(')')))
        .collect::<Vec<_>>()
        .join(" ");
    joined
        .replace("[BLANK_AUDIO]", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn confidence_of(d: &Detection) -> (f32, &'static str) {
    match d.source {
        DetectSource::Explicit => {
            if d.reference.verse.is_some() { (0.95, "explicit") } else { (0.85, "explicit") }
        }
        DetectSource::Fuzzy => {
            if d.reference.verse.is_some() { (0.82, "fuzzy") } else { (0.72, "fuzzy") }
        }
        DetectSource::Context => (0.80, "context"),
        DetectSource::Descriptive => {
            if d.reference.verse.is_some() { (0.9, "descriptive") } else { (0.8, "descriptive") }
        }
        DetectSource::Story => (0.7, "story"),
    }
}

/// Transcribe a clip, emit transcript (final only), and emit new candidates.
/// Shared by the final (endpointed) flush and the interim mid-utterance pass.
fn transcribe_detect(
    app: &AppHandle,
    model: &Path,
    binary: &Path,
    audio: &[f32],
    ctx: &mut RefContext,
    last: &mut Option<RefKey>,
    emit_transcript: bool,
) {
    let text = match stt::transcribe(audio, model, binary) {
        Ok(t) => clean_transcript(&t),
        Err(e) => {
            let _ = app.emit("listen-error", e);
            return;
        }
    };
    if text.is_empty() {
        return;
    }
    if emit_transcript {
        let _ = app.emit("transcript", text.clone());
    }

    let detections = detect::detect_with_context(&text, ctx);

    // Relative voice navigation ("next verse", "next chapter") against the
    // currently-presented scripture — the fast hands-free flow.
    if detections.is_empty() {
        if let Some(dir) = detect::detect_nav_command(&text) {
            if let Some(payload) = crate::commands::navigate_handle(app, dir) {
                let _ = app.emit(
                    "verse-candidate",
                    Candidate { verse: payload, confidence: 0.9, source: "voice-nav".into() },
                );
                return;
            }
        }
    }

    let state = app.state::<AppState>();
    // A spoken translation ("...in ASV") switches to it when installed.
    let tr = crate::commands::resolve_translation(&state, &text);
    let _ = app.emit("translation-changed", &tr);
    let candidates: Vec<Candidate> = {
        let db = match state.db.lock() {
            Ok(d) => d,
            Err(_) => return,
        };
        let mut out = Vec::new();
        if !detections.is_empty() {
            // Explicit / fuzzy / context references.
            for d in &detections {
                let r = &d.reference;
                let key: RefKey = (r.book_osis.clone(), r.chapter, r.verse);
                if last.as_ref() == Some(&key) {
                    continue;
                }
                if let Ok(Some(rec)) = db.find_verse(&tr, r) {
                    let (confidence, source) = confidence_of(d);
                    out.push(Candidate { verse: build_payload(rec), confidence, source: source.to_string() });
                    *last = Some(key);
                }
            }
        } else if let Some((query, words)) = semantic::fts_query(&text) {
            // No spoken reference — look for a quoted/paraphrased verse (FTS).
            if let Ok(hits) = db.search_fts(&tr, &query, 3) {
                if let Some((rec, _rank)) = hits.into_iter().next() {
                    let ov = semantic::overlap(&words, &rec.text);
                    let key: RefKey = (rec.book_osis.clone(), rec.chapter, Some(rec.verse));
                    if semantic::is_strong(ov, words.len()) && last.as_ref() != Some(&key) {
                        let confidence = semantic::confidence(ov, words.len());
                        out.push(Candidate { verse: build_payload(rec), confidence, source: "quote".into() });
                        *last = Some(key);
                    }
                }
            }
        }
        out
    };
    for c in candidates {
        let _ = app.emit("verse-candidate", c);
    }
}

fn run_inner(
    app: &AppHandle,
    flag: &Arc<AtomicBool>,
    model: &Path,
    binary: &Path,
) -> Result<(), String> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("no input microphone found")?;
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let channels = config.channels as usize;
    let src_rate = config.sample_rate.0 as f32;

    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let err_fn = |e| eprintln!("audio stream error: {e}");

    let build = match sample_format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| {
                let _ = tx.send(downmix_resample(data, channels, src_rate));
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _: &_| {
                let f: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                let _ = tx.send(downmix_resample(&f, channels, src_rate));
            },
            err_fn,
            None,
        ),
        other => return Err(format!("unsupported sample format {other:?}")),
    };
    let stream = build.map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    let _ = app.emit("listen-started", ());

    let mut utter: Vec<f32> = Vec::new();
    let mut recent: Vec<f32> = Vec::new();
    let mut speech_ms = 0f32;
    let mut silence_ms = 0f32;
    let mut last_interim = 0usize;
    let mut ctx = RefContext::default();
    let mut last_ref: Option<RefKey> = None;
    let mut last_activity = Instant::now();

    while flag.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(150)) {
            Ok(frame) => {
                recent.extend_from_slice(&frame);
                if recent.len() > PREROLL_SAMPLES {
                    let drop = recent.len() - PREROLL_SAMPLES;
                    recent.drain(0..drop);
                }

                let dur = ms_of(frame.len());
                if rms(&frame) > SILENCE_RMS {
                    if utter.is_empty() {
                        utter.extend_from_slice(&recent);
                        last_interim = utter.len();
                    }
                    utter.extend_from_slice(&frame);
                    speech_ms += dur;
                    silence_ms = 0.0;
                } else if !utter.is_empty() {
                    utter.extend_from_slice(&frame);
                    silence_ms += dur;
                }

                // Interim pass: only fires for long continuous speech (no pause yet).
                if speech_ms >= MIN_SPEECH_MS
                    && utter.len().saturating_sub(last_interim) >= INTERIM_SAMPLES
                {
                    transcribe_detect(app, model, binary, &utter, &mut ctx, &mut last_ref, false);
                    last_interim = utter.len();
                    last_activity = Instant::now();
                }

                let ended = !utter.is_empty() && silence_ms >= SILENCE_FLUSH_MS;
                let too_long = ms_of(utter.len()) >= MAX_UTTER_MS;
                if ended || too_long {
                    let audio = std::mem::take(&mut utter);
                    let had = speech_ms;
                    speech_ms = 0.0;
                    silence_ms = 0.0;
                    last_interim = 0;
                    if had >= MIN_SPEECH_MS {
                        if last_activity.elapsed().as_secs() > CTX_STALE_SECS {
                            ctx.clear();
                        }
                        last_activity = Instant::now();
                        transcribe_detect(app, model, binary, &audio, &mut ctx, &mut last_ref, true);
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !utter.is_empty() {
                    silence_ms += 150.0;
                    if silence_ms >= SILENCE_FLUSH_MS {
                        let audio = std::mem::take(&mut utter);
                        let had = speech_ms;
                        speech_ms = 0.0;
                        silence_ms = 0.0;
                        last_interim = 0;
                        if had >= MIN_SPEECH_MS {
                            last_activity = Instant::now();
                            transcribe_detect(app, model, binary, &audio, &mut ctx, &mut last_ref, true);
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(stream);
    Ok(())
}

pub fn run_listen_loop(app: AppHandle, flag: Arc<AtomicBool>, model: PathBuf, binary: PathBuf) {
    if let Err(e) = run_inner(&app, &flag, &model, &binary) {
        let _ = app.emit("listen-error", e);
    }
    flag.store(false, Ordering::SeqCst);
    let _ = app.emit("listen-stopped", ());
}
