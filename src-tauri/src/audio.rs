use crate::commands::{build_payload, AppState};
use crate::detect::{self, DetectSource, Detection, RefContext};
use crate::events::Candidate;
use crate::stt;
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
            if d.reference.verse.is_some() {
                (0.95, "explicit")
            } else {
                (0.85, "explicit")
            }
        }
        DetectSource::Fuzzy => {
            if d.reference.verse.is_some() {
                (0.82, "fuzzy")
            } else {
                (0.72, "fuzzy")
            }
        }
        DetectSource::Context => (0.80, "context"),
    }
}

fn handle_utterance(
    app: &AppHandle,
    model: &Path,
    binary: &Path,
    audio: Vec<f32>,
    ctx: &mut RefContext,
    last: &mut Option<RefKey>,
) {
    let text = match stt::transcribe(&audio, model, binary) {
        Ok(t) => clean_transcript(&t),
        Err(e) => {
            let _ = app.emit("listen-error", e);
            return;
        }
    };
    if text.is_empty() {
        return;
    }
    let _ = app.emit("transcript", text.clone());

    let detections = detect::detect_with_context(&text, ctx);
    if detections.is_empty() {
        return;
    }
    let state = app.state::<AppState>();
    let candidates: Vec<Candidate> = {
        let db = match state.db.lock() {
            Ok(d) => d,
            Err(_) => return,
        };
        let mut out = Vec::new();
        for d in &detections {
            let r = &d.reference;
            let key: RefKey = (r.book_osis.clone(), r.chapter, r.verse);
            if last.as_ref() == Some(&key) {
                continue;
            }
            if let Ok(Some(rec)) = db.find_verse(&state.translation, r) {
                let (confidence, source) = confidence_of(d);
                out.push(Candidate {
                    verse: build_payload(rec),
                    confidence,
                    source: source.to_string(),
                });
                *last = Some(key);
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
    let device = host
        .default_input_device()
        .ok_or("no input microphone found")?;
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
    let mut recent: Vec<f32> = Vec::new(); // rolling pre-roll buffer
    let mut speech_ms = 0f32;
    let mut silence_ms = 0f32;
    let mut ctx = RefContext::default();
    let mut last_ref: Option<RefKey> = None;
    let mut last_activity = Instant::now();

    let mut flush = |utter: &mut Vec<f32>, speech_ms: &mut f32, silence_ms: &mut f32| {
        let audio = std::mem::take(utter);
        let had_speech = *speech_ms;
        *speech_ms = 0.0;
        *silence_ms = 0.0;
        if had_speech < MIN_SPEECH_MS {
            return;
        }
        if last_activity.elapsed().as_secs() > CTX_STALE_SECS {
            ctx.clear();
        }
        last_activity = Instant::now();
        handle_utterance(app, model, binary, audio, &mut ctx, &mut last_ref);
    };

    while flag.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(150)) {
            Ok(frame) => {
                // maintain pre-roll ring
                recent.extend_from_slice(&frame);
                if recent.len() > PREROLL_SAMPLES {
                    let drop = recent.len() - PREROLL_SAMPLES;
                    recent.drain(0..drop);
                }

                let dur = ms_of(frame.len());
                if rms(&frame) > SILENCE_RMS {
                    if utter.is_empty() {
                        utter.extend_from_slice(&recent); // seed with pre-speech audio
                    }
                    utter.extend_from_slice(&frame);
                    speech_ms += dur;
                    silence_ms = 0.0;
                } else if !utter.is_empty() {
                    utter.extend_from_slice(&frame);
                    silence_ms += dur;
                }

                let ended = !utter.is_empty() && silence_ms >= SILENCE_FLUSH_MS;
                let too_long = ms_of(utter.len()) >= MAX_UTTER_MS;
                if ended || too_long {
                    flush(&mut utter, &mut speech_ms, &mut silence_ms);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !utter.is_empty() {
                    silence_ms += 150.0;
                    if silence_ms >= SILENCE_FLUSH_MS {
                        flush(&mut utter, &mut speech_ms, &mut silence_ms);
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
