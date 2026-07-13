use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Windows spawns a console window for a console subsystem binary unless told
/// not to. whisper-cli is invoked once per interim pass and once per endpoint,
/// so without this a black window flashes several times per spoken sentence.
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
            // Greedy, single-candidate decoding — much faster than the default
            // beam search, with negligible accuracy loss for short references.
            "-bs",
            "1",
            "-bo",
            "1",
            "--prompt",
            BIBLE_PROMPT,
            "-nt",
            "-otxt",
            "-of",
            base.to_str().ok_or("bad out path")?,
        ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().map_err(|e| format!("failed to run whisper binary: {e}"))?;

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
