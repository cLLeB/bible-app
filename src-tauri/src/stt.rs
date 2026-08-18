//! Speech-to-text against a resident whisper.cpp server.
//!
//! whisper-server keeps the model in memory for the life of the listening
//! session, so an utterance pays only for its own encode+decode. The old design
//! spawned whisper-cli.exe per utterance and re-read the model from disk every
//! time — 0.44s for base, ~1.4s for small, on every single pass.
//!
//! Two other costs were hiding behind that one. whisper encodes a fixed 30s
//! window regardless of clip length, so a 3s reference paid to encode 27s of
//! silence; `audio_ctx` sizes the window to the clip. And beam search, avoided
//! for speed, turns out to cost almost nothing once the encode is right.
//!
//! Measured end-to-end on one 2.8s clip (this machine):
//!     base   4.33s -> 0.34s      small  20.22s -> 1.34s
//!
//! A statically linked engine (whisper-rs) was tried and decoded ~6x slower: the
//! shipped binary picks a CPU-specific backend DLL (AVX2/FMA) at runtime, and
//! those kernels are worth far more than avoiding the process boundary.
//!
//! If the server cannot start, transcription falls back to whisper-cli.exe so
//! speech never simply stops working.

use std::io::Cursor;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, Instant};
#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Windows gives a console-subsystem binary its own console window unless told
/// otherwise. whisper runs once per interim pass and once per endpoint, so
/// without this a black window flashes several times per spoken sentence.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

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
    /// 1 = greedy. Higher searches more candidate transcripts. Costs decode time
    /// only — measured at ~60ms on base — so it is on by default.
    pub beam: i32,
    /// Prime the decoder with scripture vocabulary.
    pub prompt: bool,
    /// Scale speech to a consistent level first: a quiet mic hurts small models most.
    pub normalize: bool,
    /// Encoder window. See `Window`.
    pub window: Window,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Window {
    /// whisper's default: the full 30 seconds, however short the clip.
    Full,
    /// Sized to the clip: 50 encoder frames per second of speech, times `margin`.
    /// A very tight window can make the decoder loop and repeat the phrase (which
    /// `dedupe_repeats` collapses); too loose and the encode cost returns. What is
    /// right depends on the model — see `Decode::for_model`.
    Fit { margin: f32 },
}

impl Default for Decode {
    fn default() -> Self {
        Self { beam: 5, prompt: true, normalize: true, window: Window::Fit { margin: 1.5 } }
    }
}

impl Decode {
    /// Defaults differ by model, because the models want opposite things. Both
    /// figures below are measured on 12 recorded utterances of real speech, not
    /// on synthetic audio — which pointed the other way and was simply wrong.
    ///
    /// small wants a TIGHT window: fitted x1.5 resolves 10/12, x2 drops to 8/12,
    /// and the full window manages only 7-8/12. Trimming the encoder to the clip
    /// evidently keeps it from wandering off into the silence.
    ///
    /// tiny and base wanted the opposite, and that is why this used to branch. They
    /// are gone from the project, so it no longer does.
    pub fn for_model(_model: &Path) -> Self {
        // Every model still in the project is small or larger, and those want the
        // fitted window. The full-window branch existed for tiny and base, which are
        // no longer used anywhere: base transcribed badly enough to read as a
        // detection fault, and on a graphics card small is faster than base ever was
        // on the processor, so nothing is being traded for it.
        Self::default()
    }

    /// Bench/dev override, e.g. `BIBLE_APP_DECODE=beam=1,prompt=0,window=full`.
    pub fn from_env_or_model(model: &Path) -> Self {
        let mut d = Self::for_model(model);
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

    /// Encoder frames for `secs` of audio. whisper's encoder covers 30s in 1500
    /// frames, so 50 frames per second of speech, plus margin.
    fn audio_ctx(&self, secs: f32) -> Option<i32> {
        const FRAMES_PER_SEC: f32 = 50.0;
        const FULL: f32 = 1500.0;
        match self.window {
            Window::Full => None,
            Window::Fit { margin } => {
                Some((secs * FRAMES_PER_SEC * margin).ceil().clamp(256.0, FULL) as i32)
            }
        }
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

fn wav_bytes(samples: &[f32]) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut buf = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut buf, spec).map_err(|e| e.to_string())?;
        for &s in samples {
            let clamped = s.clamp(-1.0, 1.0);
            writer
                .write_sample((clamped * i16::MAX as f32) as i16)
                .map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}

pub fn write_wav_16k_mono(path: &Path, samples: &[f32]) -> Result<(), String> {
    std::fs::write(path, wav_bytes(samples)?).map_err(|e| e.to_string())
}

/// Tie a child process's lifetime to ours at the OS level. `shutdown()` handles
/// the orderly paths, but if we are killed outright — Task Manager, a crash — a
/// plain child would survive holding the whole model in RAM. A job object with
/// KILL_ON_JOB_CLOSE makes Windows reap it for us.
#[cfg(windows)]
fn tie_to_our_lifetime(child: &Child) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
        JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    // SAFETY: handles come from the OS; the job handle is deliberately leaked so
    // it lives as long as this process, which is what ties the child to us.
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE);
    }
}

#[cfg(not(windows))]
fn tie_to_our_lifetime(_child: &Child) {}

/// The device whisper reported loading onto, as it named it — "Intel(R) Iris(R) Xe
/// Graphics", "NVIDIA GeForce RTX 4060", or None while nothing is running.
static DEVICE: Mutex<Option<String>> = Mutex::new(None);

/// What is actually doing the work right now, in whisper's own words. None means no
/// server is running, or it did not say — in which case it is on the processor.
pub fn device() -> Option<String> {
    DEVICE.lock().ok().and_then(|g| g.clone())
}

/// Pick the device name out of a line of whisper's startup chatter.
///
/// Each backend announces itself differently, and the exact wording is upstream's to
/// change, so this is deliberately forgiving: anything it does not recognise simply
/// leaves the device unknown, which reads as "processor" and is the safe thing to say.
pub(crate) fn parse_device(line: &str) -> Option<String> {
    // "ggml_vulkan: 0 = Intel(R) Iris(R) Xe Graphics (Intel Corporation) | uma: 1 | ..."
    if let Some(rest) = line.strip_prefix("ggml_vulkan:") {
        let after_eq = rest.split_once('=')?.1.trim();
        let name = after_eq.split('|').next()?.trim();
        // Trim the vendor in brackets at the end, keeping any inside the name itself.
        let name = name.rsplit_once(" (").map(|(n, _)| n).unwrap_or(name);
        return (!name.is_empty()).then(|| name.to_string());
    }
    // "ggml_cuda: Device 0: NVIDIA GeForce RTX 4060, compute capability 8.9, ..."
    if line.starts_with("ggml_cuda:") {
        if let Some((_, rest)) = line.split_once("Device ") {
            let name = rest.split_once(':').map(|(_, n)| n).unwrap_or(rest);
            let name = name.split(',').next()?.trim();
            return (!name.is_empty()).then(|| name.to_string());
        }
    }
    // "ggml_metal_init: picking default device: Apple M2 Pro"
    if let Some((_, name)) = line.split_once("picking default device:") {
        let name = name.trim();
        return (!name.is_empty()).then(|| name.to_string());
    }
    None
}

/// Read the server's startup output, note which device it landed on, and keep the
/// pipe drained.
///
/// Draining matters as much as reading: a piped child that nobody reads from blocks
/// once the pipe buffer fills, which would hang transcription solid. So the thread
/// keeps consuming for the life of the process and simply discards what it does not
/// need.
fn watch_startup_output(child: &mut Child) {
    let Some(stderr) = child.stderr.take() else { return };
    std::thread::spawn(move || {
        use std::io::BufRead;
        if let Ok(mut guard) = DEVICE.lock() {
            *guard = None;
        }
        for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Some(name) = parse_device(&line) {
                if let Ok(mut guard) = DEVICE.lock() {
                    *guard = Some(name);
                }
            }
        }
    });
}

/// The running whisper-server: model loaded, waiting for audio.
struct Server {
    child: Child,
    port: u16,
    model: PathBuf,
}

impl Server {
    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

static SERVER: Mutex<Option<Server>> = Mutex::new(None);

fn free_port() -> Result<u16, String> {
    let sock = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    sock.local_addr().map(|a| a.port()).map_err(|e| e.to_string())
}

/// How many threads to give whisper.
///
/// ggml puts a barrier between every layer, so a pass runs at the speed of its
/// slowest thread. On the hybrid Intel chips now in most laptops (a few fast
/// performance cores plus a bank of slow efficiency ones) that makes the count a
/// real tuning knob rather than "use everything": threads landing on E-cores can
/// hold back the ones on P-cores.
///
/// 8 is where this has sat, and it is a guess, not a measurement — a sweep on this
/// machine was inconclusive because the laptop was 69% busy with a browser at the
/// time, which itself cost more than any thread count could win back. So the number
/// is now settable rather than silently assumed:
///
///   * `BIBLE_APP_THREADS` for benching and for an operator working around a
///     machine that behaves oddly,
///   * otherwise whatever a measurement on this machine found (`set_tuned_threads`,
///     fed by the accelerator probe),
///   * otherwise the old default, so nothing regresses on a machine that has not
///     been measured.
pub fn threads() -> String {
    if let Ok(v) = std::env::var("BIBLE_APP_THREADS") {
        if let Ok(n) = v.trim().parse::<usize>() {
            if n > 0 {
                return n.to_string();
            }
        }
    }
    if let Some(n) = tuned_threads() {
        return n.to_string();
    }
    default_threads().to_string()
}

fn default_threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get().min(8)).unwrap_or(4)
}

/// The thread count measured for this machine, set once by `calibrate::tune_threads`.
static TUNED_THREADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn tuned_threads() -> Option<usize> {
    match TUNED_THREADS.load(std::sync::atomic::Ordering::Relaxed) {
        0 => None,
        n => Some(n),
    }
}

pub fn set_tuned_threads(n: usize) {
    TUNED_THREADS.store(n, std::sync::atomic::Ordering::Relaxed);
}

/// The thread counts worth trying on this machine, cheapest sweep that still
/// brackets the answer: half the cores, the current default, and everything.
pub fn thread_candidates() -> Vec<usize> {
    let all = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let mut c = vec![(all / 2).max(2), default_threads(), all];
    c.sort_unstable();
    c.dedup();
    c
}

/// Start whisper-server for `model` (or reuse the running one). The server loads
/// the model before it accepts connections, so a successful connect means ready.
fn ensure_server(model: &Path, server_exe: &Path) -> Result<u16, String> {
    let mut guard = SERVER.lock().map_err(|e| e.to_string())?;

    if let Some(srv) = guard.as_mut() {
        if srv.model == model && srv.alive() {
            return Ok(srv.port);
        }
        let _ = srv.child.kill();
        *guard = None;
    }

    let port = free_port()?;
    let mut cmd = Command::new(server_exe);
    cmd.args([
        "-m",
        model.to_str().ok_or("bad model path")?,
        "--host",
        "127.0.0.1",
        "--port",
        &port.to_string(),
        "-t",
        &threads(),
        "-nt",
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.stdout(std::process::Stdio::null());
    // Kept rather than discarded, because whisper announces here which device it
    // actually loaded onto — and that is the only trustworthy answer to "is the
    // graphics card being used". What backend the app *chose* is a belief; this is
    // the engine reporting what it did.
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("could not start whisper-server: {e}"))?;
    tie_to_our_lifetime(&child);
    watch_startup_output(&mut child);

    // The server binds its port before it has finished reading the model — half a
    // gigabyte, for small — so an open port does not mean it can answer. Firing an
    // utterance at it in that window times out, falls back to the slow path, and
    // spawns another server on the next one. So ask it something and wait for it to
    // actually reply.
    let deadline = Instant::now() + Duration::from_secs(180);
    while Instant::now() < deadline {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            let probe = ureq::get(&format!("http://127.0.0.1:{port}/"))
                .timeout(Duration::from_secs(2))
                .call();
            // Any HTTP answer at all — even a 404 — means the model is loaded and it
            // is serving. Only a transport error means it is still busy starting.
            let serving = match probe {
                Ok(_) => true,
                Err(ureq::Error::Status(_, _)) => true,
                Err(_) => false,
            };
            if serving {
                *guard = Some(Server { child, port, model: model.to_path_buf() });
                return Ok(port);
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let mut child = child;
    let _ = child.kill();
    let _ = child.wait();
    Err("whisper-server did not come up".into())
}

/// Load the model now, in the background, instead of on the first thing anyone says.
///
/// `ensure_server` is called from inside `transcribe`, so without this the first
/// utterance of the service waits for half a gigabyte to be read off disk on top of
/// its own transcription — measured at 2.3s to 4s for `small`. That is the very
/// first impression the app gives, every Sunday, and it was the worst moment of the
/// whole service.
///
/// Safe to call more than once: `ensure_server` reuses a server already running for
/// the same model. A failure is not reported to the operator, because nothing is
/// broken — the first utterance simply loads the model the old way.
pub fn prewarm(model: &Path, binary: &Path) {
    let server_exe = binary.with_file_name("whisper-server.exe");
    if !server_exe.exists() {
        return;
    }
    let model = model.to_path_buf();
    std::thread::spawn(move || {
        if let Err(e) = ensure_server(&model, &server_exe) {
            eprintln!("whisper prewarm failed ({e}); the first utterance will load the model");
        }
    });
}

/// Stop the resident server and free its memory. Called when listening stops and
/// when the app exits — otherwise a model-sized chunk of RAM lingers.
pub fn shutdown() {
    if let Ok(mut guard) = SERVER.lock() {
        if let Some(srv) = guard.as_mut() {
            let _ = srv.child.kill();
            let _ = srv.child.wait();
        }
        *guard = None;
    }
}

fn multipart(wav: &[u8], fields: &[(&str, String)]) -> (String, Vec<u8>) {
    let boundary = format!("----bibleapp{}", std::process::id());
    let mut body = Vec::new();
    for (k, v) in fields {
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n")
                .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"utt.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(wav);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (boundary, body)
}

/// Transcribe 16 kHz mono f32 samples. `binary` is the whisper-cli path; the
/// server is expected beside it.
pub fn transcribe(
    samples16k: &[f32],
    model: &Path,
    binary: &Path,
    decode: Decode,
) -> Result<String, String> {
    let audio = if decode.normalize { normalize(samples16k) } else { samples16k.to_vec() };
    let secs = audio.len() as f32 / 16_000.0;
    let wav = wav_bytes(&audio)?;

    let server_exe = binary.with_file_name("whisper-server.exe");
    if server_exe.exists() {
        match transcribe_via_server(&wav, secs, model, &server_exe, decode) {
            Ok(text) => return Ok(dedupe_repeats(&text)),
            Err(e) => {
                // A dead server must not mean no speech: fall back to the CLI,
                // and let the next utterance try to start it again.
                eprintln!("whisper-server failed ({e}); falling back to whisper-cli");
                shutdown();
            }
        }
    }
    transcribe_via_cli(&wav, secs, model, binary, decode).map(|t| dedupe_repeats(&t))
}

fn transcribe_via_server(
    wav: &[u8],
    secs: f32,
    model: &Path,
    server_exe: &Path,
    decode: Decode,
) -> Result<String, String> {
    let port = ensure_server(model, server_exe)?;

    let mut fields: Vec<(&str, String)> = vec![
        ("response_format", "text".into()),
        ("language", "en".into()),
        ("beam_size", decode.beam.to_string()),
        ("best_of", decode.beam.to_string()),
    ];
    if let Some(ctx) = decode.audio_ctx(secs) {
        fields.push(("audio_ctx", ctx.to_string()));
    }
    if decode.prompt {
        fields.push(("prompt", BIBLE_PROMPT.into()));
    }
    let (boundary, body) = multipart(wav, &fields);

    let resp = ureq::post(&format!("http://127.0.0.1:{port}/inference"))
        .set("Content-Type", &format!("multipart/form-data; boundary={boundary}"))
        .timeout(Duration::from_secs(120))
        .send_bytes(&body)
        .map_err(|e| e.to_string())?;
    resp.into_string().map(|t| t.trim().to_string()).map_err(|e| e.to_string())
}

/// The whisper-cli flags that carry one `Decode`, for a clip of `secs`.
///
/// Shared with the accelerator probe rather than written out twice. A probe that
/// decodes differently from a service measures the wrong thing: the encoder window
/// alone is worth 4x on this machine (7.2s fitted against 29.7s full), and it shifts
/// the balance between encode and decode — which is exactly the balance that decides
/// whether a graphics card is worth using.
pub fn cli_decode_args(decode: Decode, secs: f32) -> Vec<String> {
    let beam = decode.beam.to_string();
    let mut args = vec!["-bs".into(), beam.clone(), "-bo".into(), beam];
    if let Some(ctx) = decode.audio_ctx(secs) {
        args.push("-ac".into());
        args.push(ctx.to_string());
    }
    if decode.prompt {
        args.push("--prompt".into());
        args.push(BIBLE_PROMPT.into());
    }
    args
}

/// Fallback: one-shot whisper-cli. Correct but slower — it reloads the model.
fn transcribe_via_cli(
    wav: &[u8],
    secs: f32,
    model: &Path,
    binary: &Path,
    decode: Decode,
) -> Result<String, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let base = std::env::temp_dir().join(format!("bibleapp_utt_{stamp}"));
    let wav_path = base.with_extension("wav");
    std::fs::write(&wav_path, wav).map_err(|e| e.to_string())?;

    let mut cmd = Command::new(binary);
    cmd.args([
        "-m",
        model.to_str().ok_or("bad model path")?,
        "-f",
        wav_path.to_str().ok_or("bad wav path")?,
        "-l",
        "en",
        "-t",
        &threads(),
        "-nt",
        "-otxt",
        "-of",
        base.to_str().ok_or("bad out path")?,
    ]);
    cmd.args(cli_decode_args(decode, secs));
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd.output().map_err(|e| format!("failed to run whisper binary: {e}"))?;
    let txt_path = base.with_extension("txt");
    let result = std::fs::read_to_string(&txt_path).map(|t| t.trim().to_string());
    let _ = std::fs::remove_file(&wav_path);
    let _ = std::fs::remove_file(&txt_path);

    result.map_err(|_| {
        format!("whisper produced no output: {}", String::from_utf8_lossy(&out.stderr))
    })
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
    let key = |w: &str| {
        w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
    };
    for len in (2..=n / 2).rev() {
        let (a, b) = (&words[n - 2 * len..n - len], &words[n - len..]);
        if a.iter().zip(b.iter()).all(|(x, y)| key(x) == key(y)) {
            return words[..n - len].join(" ");
        }
    }
    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real startup lines, copied from this machine and from upstream's output. The
    /// operator is told which processor is doing the work, and this is where that
    /// answer comes from, so a silent parsing failure would mean quietly reporting
    /// "processor" while the graphics card is busy.
    #[test]
    fn reads_the_device_whisper_says_it_loaded_onto() {
        assert_eq!(
            parse_device(
                "ggml_vulkan: 0 = Intel(R) Iris(R) Xe Graphics (Intel Corporation) | uma: 1 | fp16: 1"
            )
            .as_deref(),
            Some("Intel(R) Iris(R) Xe Graphics")
        );
        assert_eq!(
            parse_device("ggml_cuda: Device 0: NVIDIA GeForce RTX 4060, compute capability 8.9")
                .as_deref(),
            Some("NVIDIA GeForce RTX 4060")
        );
        assert_eq!(
            parse_device("ggml_metal_init: picking default device: Apple M2 Pro").as_deref(),
            Some("Apple M2 Pro")
        );
    }

    #[test]
    fn ordinary_chatter_does_not_become_a_device_name() {
        // Unknown wording must leave the device unset, which reads as "processor".
        // Claiming a graphics card that is not there would be worse than saying
        // nothing.
        for line in [
            "whisper_model_load: n_vocab = 51864",
            "system_info: n_threads = 8",
            "",
            "ggml_vulkan: Found 1 Vulkan devices:",
        ] {
            assert_eq!(parse_device(line), None, "should not parse: {line}");
        }
    }

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
        let d = Decode::default(); // Fit { margin: 1.5 }
        assert_eq!(d.audio_ctx(3.0), Some(256)); // 3s * 50 * 1.5 = 225 -> floor
        assert_eq!(d.audio_ctx(0.5), Some(256)); // floor
        assert_eq!(d.audio_ctx(60.0), Some(1500)); // never exceeds whisper's window
        assert_eq!(Decode { window: Window::Full, ..d }.audio_ctx(3.0), None);
    }

    #[test]
    fn every_model_still_in_the_project_fits_the_window_to_the_clip() {
        for m in ["ggml-small.en.bin", "ggml-medium.en.bin"] {
            assert_eq!(Decode::for_model(Path::new(m)).window, Window::Fit { margin: 1.5 }, "{m}");
        }
    }

    #[test]
    fn wav_round_trips_at_16k_mono() {
        let bytes = wav_bytes(&[0.0, 0.5, -0.5]).unwrap();
        let reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().channels, 1);
    }
}
