use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

// Bias whisper toward scripture vocabulary (book names + "chapter"/"verse").
const BIBLE_PROMPT: &str = "A spoken Bible scripture reference, for example John chapter 3 verse 16 or Romans 8:28. \
Books: Genesis, Exodus, Leviticus, Numbers, Deuteronomy, Joshua, Judges, Ruth, 1 Samuel, 2 Samuel, 1 Kings, 2 Kings, \
1 Chronicles, 2 Chronicles, Ezra, Nehemiah, Esther, Job, Psalms, Proverbs, Ecclesiastes, Song of Solomon, Isaiah, \
Jeremiah, Lamentations, Ezekiel, Daniel, Hosea, Joel, Amos, Obadiah, Jonah, Micah, Nahum, Habakkuk, Zephaniah, Haggai, \
Zechariah, Malachi, Matthew, Mark, Luke, John, Acts, Romans, 1 Corinthians, 2 Corinthians, Galatians, Ephesians, \
Philippians, Colossians, 1 Thessalonians, 2 Thessalonians, 1 Timothy, 2 Timothy, Titus, Philemon, Hebrews, James, \
1 Peter, 2 Peter, 1 John, 2 John, 3 John, Jude, Revelation. Chapter and verse.";

fn temp_base() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("bibleapp_utt_{ts}_{n}"))
}

fn write_wav_16k_mono(path: &Path, samples: &[f32]) -> Result<(), String> {
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

/// Transcribe 16kHz mono f32 samples by invoking a whisper.cpp binary.
pub fn transcribe(samples16k: &[f32], model: &Path, binary: &Path) -> Result<String, String> {
    let base = temp_base();
    let wav_path = base.with_extension("wav");
    write_wav_16k_mono(&wav_path, samples16k)?;

    let threads = std::thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(4)
        .to_string();

    let out = Command::new(binary)
        .args([
            "-m",
            model.to_str().ok_or("bad model path")?,
            "-f",
            wav_path.to_str().ok_or("bad wav path")?,
            "-l",
            "en",
            "-t",
            &threads,
            "--prompt",
            BIBLE_PROMPT,
            "-nt",
            "-otxt",
            "-of",
            base.to_str().ok_or("bad out path")?,
        ])
        .output()
        .map_err(|e| format!("failed to run whisper binary: {e}"))?;

    let txt_path = base.with_extension("txt");
    let result = std::fs::read_to_string(&txt_path).map(|t| t.trim().to_string());

    let _ = std::fs::remove_file(&wav_path);
    let _ = std::fs::remove_file(&txt_path);

    match result {
        Ok(text) => Ok(text),
        Err(_) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Err(format!("whisper produced no output: {stderr}"))
        }
    }
}
