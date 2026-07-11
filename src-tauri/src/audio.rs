use crate::commands::{build_payload, AppState};
use crate::events::VersePayload;
use crate::{detect, stt};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const TARGET_RATE: u32 = 16_000;
const SILENCE_RMS: f32 = 0.012; // below this = silence
const SILENCE_FLUSH_MS: f32 = 800.0; // pause that ends an utterance
const MIN_UTTER_SAMPLES: usize = (TARGET_RATE as usize) / 2; // ignore < 0.5s blips

fn rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum: f32 = frame.iter().map(|s| s * s).sum();
    (sum / frame.len() as f32).sqrt()
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

fn process_utterance(app: &AppHandle, model: &Path, binary: &Path, audio: Vec<f32>) {
    if audio.len() < MIN_UTTER_SAMPLES {
        return;
    }
    let text = match stt::transcribe(&audio, model, binary) {
        Ok(t) => t,
        Err(e) => {
            let _ = app.emit("listen-error", e);
            return;
        }
    };
    if text.is_empty() {
        return;
    }
    let _ = app.emit("transcript", text.clone());

    let refs = detect::detect_references(&text);
    if refs.is_empty() {
        return;
    }
    let state = app.state::<AppState>();
    let payloads: Vec<VersePayload> = {
        let db = match state.db.lock() {
            Ok(d) => d,
            Err(_) => return,
        };
        refs.iter()
            .filter_map(|r| db.find_verse(&state.translation, r).ok().flatten())
            .map(build_payload)
            .collect()
    };
    for p in payloads {
        let _ = app.emit("verse-candidate", p);
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
    let mut silence_ms = 0f32;

    while flag.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(150)) {
            Ok(frame) => {
                let dur_ms = frame.len() as f32 / TARGET_RATE as f32 * 1000.0;
                if rms(&frame) > SILENCE_RMS {
                    utter.extend_from_slice(&frame);
                    silence_ms = 0.0;
                } else if !utter.is_empty() {
                    utter.extend_from_slice(&frame);
                    silence_ms += dur_ms;
                }
                if !utter.is_empty() && silence_ms >= SILENCE_FLUSH_MS {
                    process_utterance(app, model, binary, std::mem::take(&mut utter));
                    silence_ms = 0.0;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !utter.is_empty() {
                    silence_ms += 150.0;
                    if silence_ms >= SILENCE_FLUSH_MS {
                        process_utterance(app, model, binary, std::mem::take(&mut utter));
                        silence_ms = 0.0;
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
