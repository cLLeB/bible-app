//! Teach the app a preacher from a recording of them preaching.
//!
//! Calibration asks someone to read twelve lines at a laptop. That tunes the app for
//! a voice reading a script into a microphone — not for a preacher, in a pulpit,
//! through a sound desk, saying things the way they actually say them. This does the
//! real thing: feed it a sermon and it learns from how that person preaches.
//!
//! Ground truth comes free, from how a service already works. The preacher names a
//! reference, someone projects it, and the preacher reads it off the screen. So any
//! passage read aloud in the recording is a verse that was correct that day — no
//! annotation needed from anybody.
//!
//! From that the app learns two things, and both matter:
//!
//!   * DECODE SETTINGS — which recognizer settings recover the most scripture from
//!     THIS voice on THIS signal path. The window setting alone has swung accuracy
//!     from 10/12 to 3/12 between voices, so this is not a detail.
//!
//!   * BOOK NAMES AS THIS PERSON IS MISHEARD — whisper drops the leading consonant of
//!     "Nehemiah" for one speaker and mangles "Philippians" for another. When a
//!     reference fails but the verse read out immediately afterwards identifies the
//!     book, the garbled word is worth remembering. Similarity matching cannot do
//!     this: it scores "hemaiah" closest to Zephaniah.
//!
//! Everything stays on the machine, and everything is stored against one speaker, so
//! learning a guest never disturbs the regular preacher.

use crate::db::Db;
use crate::stt::{Decode, Window};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Settings tried against the recordings. Kept small: each one re-decodes every clip.
/// Gain is swept too — it is forced on for everyone today, and nobody has ever checked
/// whether it helps a particular voice on a particular feed.
pub fn candidates() -> Vec<Decode> {
    let mut out = Vec::new();
    for window in [
        Window::Full,
        Window::Fit { margin: 1.2 },
        Window::Fit { margin: 1.5 },
        Window::Fit { margin: 2.0 },
    ] {
        for beam in [5, 1] {
            out.push(Decode { beam, prompt: true, normalize: true, window });
        }
    }
    // Same window and search, without the gain — a desk feed is often loud enough that
    // normalising only lifts the room noise with it.
    out.push(Decode { beam: 5, prompt: true, normalize: false, window: Window::Full });
    out.push(Decode { beam: 5, prompt: true, normalize: false, window: Window::Fit { margin: 1.5 } });
    out
}

/// How this speaker's audio actually behaves, learned from their recordings.
///
/// The listener decides "speaking" or "paused" against a single hardcoded loudness,
/// which assumes every church sounds the same. A gated desk feed sits near silence
/// between words; a hot one never gets that quiet. Get it wrong and the app either
/// chops a sentence in half or never decides she has finished.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Room {
    /// Loudness above which we call it speech.
    pub speech_above: f32,
}

impl Default for Room {
    fn default() -> Self {
        Self { speech_above: 0.010 } // what the app has always used
    }
}

/// Measure the room from the audio itself: find the quiet floor, and set the speech
/// threshold above it — high enough to ignore the floor, low enough to catch the ends
/// of words where the voice trails away.
pub fn measure_room(samples: &[f32]) -> Room {
    const FRAME: usize = 160; // 10 ms at 16 kHz
    let mut levels: Vec<f32> = samples
        .chunks(FRAME)
        .map(|f| (f.iter().map(|s| s * s).sum::<f32>() / f.len().max(1) as f32).sqrt())
        .collect();
    if levels.len() < 100 {
        return Room::default();
    }
    levels.sort_by(f32::total_cmp);
    let floor = levels[levels.len() / 20]; // 5th percentile: the room between words
    let speech = levels[levels.len() * 9 / 10]; // 90th: the voice itself

    // Sit well clear of the floor but far below the voice. Clamped so a broken
    // measurement can never make the app deaf or leave it hearing its own hiss.
    let threshold = (floor * 4.0).max(speech * 0.06).clamp(0.002, 0.05);
    Room { speech_above: threshold }
}

pub fn save_room(db: &Db, profile: &str, room: &Room) -> rusqlite::Result<()> {
    db.set_setting(&format!("room:{profile}"), &format!("{}", room.speech_above))
}

pub fn load_room(db: &Db, profile: &str) -> Room {
    db.get_setting(&format!("room:{profile}"))
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| (0.002..=0.05).contains(v))
        .map(|speech_above| Room { speech_above })
        .unwrap_or_default()
}

/// Which translation is this preacher reading from? The wording of a verse read aloud
/// identifies the version, so a church that reads NKJV can have NKJV projected for
/// them without anybody choosing it from a menu.
pub fn save_translation(db: &Db, profile: &str, code: &str) -> rusqlite::Result<()> {
    db.set_setting(&format!("version:{profile}"), code)
}

pub fn load_translation(db: &Db, profile: &str) -> Option<String> {
    db.get_setting(&format!("version:{profile}"))
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub stage: String,
    pub done: usize,
    pub total: usize,
    /// Whole job, 0..100 — listening and comparing are weighted by what they cost.
    pub percent: u8,
    /// Rough seconds left, from how fast it has actually been going.
    pub seconds_left: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LearnResult {
    pub profile: String,
    pub recordings: usize,
    pub minutes: f32,
    /// The version they read from, if their readings agree on one.
    pub translation: Option<String>,
    /// The speech threshold measured from their audio.
    pub speech_above: f32,
    /// Verses the preacher read aloud — what a human operator got right that day.
    pub references_found: usize,
    /// How many of those the shipped defaults would have caught.
    pub before: usize,
    /// How many the settings we picked catch.
    pub after: usize,
    pub settings: String,
    /// Book names this speaker gets misheard as, learned from the recording.
    pub learned_names: Vec<String>,
}

/// Distinctive words of a verse — the ones that make it that verse and not another.
fn key_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4)
        .map(|w| w.to_lowercase())
        .filter(|w| !crate::semantic::is_filler(w))
        .collect()
}

/// Is this utterance the speaker *reading a verse out* — most of the verse's
/// distinctive words present — rather than alluding to it? Returns the verse.
pub fn reading_of(db: &Db, text: &str) -> Option<crate::db::VerseRecord> {
    let (query, words) = crate::semantic::fts_query(text)?;
    let hits = db.search_fts_any(&query, 8).ok()?;
    let spoken: HashSet<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4)
        .map(|w| w.to_lowercase())
        .collect();
    let _ = words;
    hits.into_iter().map(|(rec, _)| rec).find(|rec| {
        let key = key_words(&rec.text);
        if key.len() < 5 {
            return false;
        }
        let said = key.iter().filter(|w| spoken.contains(*w)).count();
        said >= 5 && said as f32 / key.len() as f32 >= 0.65
    })
}

/// Does this transcript resolve to the verse (or at least the chapter) we expect?
pub fn resolves_to(text: &str, osis: &str, chapter: u16) -> bool {
    let mut ctx = crate::detect::RefContext::default();
    crate::detect::detect_with_context(text, &mut ctx)
        .iter()
        .any(|h| h.reference.book_osis == osis && h.reference.chapter == chapter)
}

/// A word this speaker's book name gets heard as.
///
/// Only learned when the evidence is unambiguous: the utterance names a chapter that
/// matches a passage the speaker went on to read out, the word where the book should
/// be resolves to no book at all, and it is not an ordinary English word we would be
/// reckless to bind to a book of the Bible.
pub fn learn_book_name(text: &str, osis: &str, chapter: u16) -> Option<String> {
    let tokens: Vec<String> = text
        .split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();

    // Find "<word> chapter <chapter>" or "<word> <chapter>" where <word> is not a book.
    for (i, tok) in tokens.iter().enumerate() {
        let mut j = i + 1;
        if tokens.get(j).map(|t| t == "chapter").unwrap_or(false) {
            j += 1;
        }
        let names_chapter = tokens
            .get(j)
            .and_then(|t| t.parse::<u16>().ok())
            .map(|n| n == chapter)
            .unwrap_or(false);
        if !names_chapter {
            continue;
        }
        if tok.len() < 5 || tok.parse::<u16>().is_ok() {
            continue;
        }
        if crate::books::resolve_book_fuzzy(tok).is_some() {
            continue; // already understood
        }
        if COMMON_WORDS.contains(&tok.as_str()) {
            continue; // never bind an everyday word to a book
        }
        if sounds_structural(tok) {
            // "chapter" itself gets mangled ("shapte") and then sits exactly where a
            // book would, right before the number. That is not a misheard book.
            continue;
        }
        if tokens[..i].iter().any(|w| crate::books::resolve_book_fuzzy(w).is_some()) {
            // The real book was already heard in this phrase, so the word before the
            // number is structural ("Romans [chapter] 8"), not the book.
            continue;
        }
        let _ = osis;
        return Some(tok.clone());
    }
    None
}

/// How close a word must sound to "chapter"/"verse" to be treated as one of them
/// rather than a misheard book. Tuned so manglings like "shapte" are caught while
/// genuine book manglings ("nemayer", "hemaiah") are not.
const STRUCTURAL_SIMILARITY: f64 = 0.80;

/// Does this word sound like a structural cue ("chapter"/"verse") rather than a book?
/// whisper mangles these too, and a mangled "chapter" lands right where a book would.
fn sounds_structural(word: &str) -> bool {
    ["chapter", "chapters", "verse", "verses"]
        .iter()
        .any(|s| strsim::jaro_winkler(word, s) >= STRUCTURAL_SIMILARITY)
}

/// Words too ordinary to ever mean a book of the Bible, however they are heard.
const COMMON_WORDS: &[&str] = &[
    "chapter", "verse", "verses", "scripture", "scriptures", "bible", "reading", "read",
    "these", "those", "which", "there", "their", "where", "about", "again", "brother",
    "sister", "church", "today", "going", "would", "could", "should", "because", "before",
    "after", "father", "spirit", "jesus", "christ", "please", "everyone", "somebody",
];

pub fn save_book_name(db: &Db, profile: &str, heard: &str, osis: &str) -> rusqlite::Result<()> {
    db.set_setting(&format!("alias:{profile}:{heard}"), osis)
}

/// Book names learned for this speaker: heard word → OSIS.
pub fn book_names(db: &Db, profile: &str) -> HashMap<String, String> {
    db.settings_with_prefix(&format!("alias:{profile}:"))
        .into_iter()
        .filter_map(|(k, v)| k.rsplit(':').next().map(|w| (w.to_string(), v)))
        .collect()
}

/// Winning settings for this speaker, in the same shape calibration stores.
pub fn save_settings(db: &Db, model: &Path, profile: &str, d: &Decode) -> rusqlite::Result<()> {
    crate::calibrate::save(db, model, profile, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learns_a_book_name_only_when_the_chapter_confirms_it() {
        // A mangling we do not know, said just before reading Nehemiah 8 — the chapter
        // vouches for it, so it is worth remembering for this speaker.
        assert_eq!(
            learn_book_name("let's read nemayer chapter 8 verse 10", "Neh", 8),
            Some("nemayer".into())
        );
        // The chapter does not match what was read: no evidence, learn nothing.
        assert_eq!(learn_book_name("let's read nemayer chapter 3", "Neh", 8), None);
        // Already understood — including manglings we have since learned the hard way,
        // like "hemaiah" — so there is nothing to add.
        assert_eq!(learn_book_name("let's read nehemiah chapter 8", "Neh", 8), None);
        assert_eq!(learn_book_name("let's read hemaiah chapter 8", "Neh", 8), None);
        // An everyday word must never become a book of the Bible.
        assert_eq!(learn_book_name("look at these chapter 8", "Neh", 8), None);
    }

    #[test]
    fn a_mangled_chapter_word_is_not_learned_as_a_book() {
        // "Romans chapter 8" heard as "romans shapte 8": Romans was in fact caught,
        // and "shapte" is a mangled "chapter" sitting where the book would be. The
        // real book is present, so nothing should be learned.
        assert_eq!(learn_book_name("romans shapte 8", "Rom", 8), None);
        // Even with the book dropped entirely, a chapter-mangling before the number
        // must not become a book.
        assert_eq!(learn_book_name("shapte 8", "Rom", 8), None);
        assert!(sounds_structural("shapte"), "must recognise a mangled 'chapter'");
        // A genuine book mangling, with no real book already heard, is still learned.
        assert_eq!(learn_book_name("read nemayer chapter 8", "Neh", 8), Some("nemayer".into()));
        assert!(!sounds_structural("nemayer"), "a real book mangling is not structural");
    }

    #[test]
    fn the_room_is_measured_between_the_words_not_guessed() {
        // A quiet feed: a near-silent floor with speech well above it.
        let quiet: Vec<f32> = (0..32_000)
            .map(|i| if i % 100 < 40 { 0.3 } else { 0.0005 })
            .collect();
        let room = measure_room(&quiet);
        assert!(room.speech_above > 0.002, "must sit above the floor");
        assert!(room.speech_above < 0.3, "must sit below the voice");

        // Too little audio to judge: keep what has always worked.
        assert_eq!(measure_room(&[0.0; 10]), Room::default());
    }

    #[test]
    fn a_transcript_resolves_to_its_chapter() {
        assert!(resolves_to("romans chapter 8 verse 28", "Rom", 8));
        assert!(!resolves_to("romans chapter 8 verse 28", "Ps", 23));
    }
}

/// Decode a sermon recording (mp3, m4a, wav, flac) to the 16 kHz mono the recognizer
/// wants. No ffmpeg, no internet, no tooling on the church's machine.
pub fn decode_audio_file(path: &Path) -> Result<Vec<f32>, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file =
        std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("unrecognized audio file: {e}"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or("no audio track in that file")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("cannot decode that audio: {e}"))?;

    let mut samples: Vec<f32> = Vec::new();
    let mut rate = 0u32;
    let mut channels = 1usize;
    let mut buf: Option<SampleBuffer<f32>> = None;

    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            // A corrupt frame in a long recording is not worth abandoning the sermon for.
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode failed: {e}")),
        };
        let spec = *decoded.spec();
        rate = spec.rate;
        channels = spec.channels.count();
        let sb = buf.get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, spec));
        sb.copy_interleaved_ref(decoded);
        samples.extend_from_slice(sb.samples());
    }
    if samples.is_empty() || rate == 0 {
        return Err("that file contains no audio".into());
    }
    // The same downmix and resample the microphone path uses, so a recording and a
    // live feed are heard identically.
    Ok(crate::audio::to_16k_mono(&samples, channels, rate as f32))
}

/// Everything a learning pass discovers about a speaker. The pass writes nothing
/// itself — the caller persists it (the app into its database, `learn_cli` into a
/// seed file baked into the installer), so both routes learn identically.
#[derive(Clone, Debug)]
pub struct Learned {
    /// Winning decode settings for `target_model`.
    pub decode: Decode,
    /// Speech threshold measured from the audio.
    pub room: Room,
    /// The version their readings matched, if one clearly won.
    pub translation: Option<String>,
    /// Misheard word -> OSIS book, learned from this speaker.
    pub aliases: Vec<(String, String)>,
    /// Passages read aloud — the ground truth a human operator got right that day.
    pub references_found: usize,
    /// How many of those the shipped defaults recover.
    pub before: usize,
    /// How many the winning settings recover.
    pub after: usize,
    /// How many the settings already in force for this speaker recover — the honest
    /// comparison when re-learning someone the app is already tuned for. `None` when
    /// there was no incumbent to measure (a speaker being learned for the first time).
    pub incumbent: Option<usize>,
    pub minutes: f32,
    pub recordings: usize,
}

/// A pass that stopped because it was asked to, not because anything went wrong. The
/// caller can try again later; nothing has been changed either way.
pub const CANCELLED: &str = "cancelled";

/// The one learning pass, shared by the in-app wizard and the offline `learn_cli`,
/// so a profile baked into the installer is the same profile the app would derive
/// from the same audio. There is deliberately no second copy of this logic.
///
/// `scout_model` finds what was read aloud — it only has to be fast, and listens to
/// every second of every sermon. `target_model` is the one the operator will
/// actually use, so it is the one whose settings get scored. `reading(text)` looks a
/// transcript up in the scripture library; taking it as a closure keeps this free of
/// the database lock, so the app can hold the lock only for each brief lookup rather
/// than for the whole hours-long pass. `say(stage, done, total, base, share)` reports
/// progress: `base` is the fraction already complete, `share` this stage's slice.
///
/// `incumbent` is the settings this speaker is already tuned with, if any. They are
/// scored alongside the candidates on the same audio, so a re-learn can answer the
/// only question that matters — is this actually better than what we have? — rather
/// than only comparing against the shipped defaults.
///
/// `keep_going()` is checked between clips: when it turns false the pass abandons what
/// it was doing and returns [`CANCELLED`]. Learning in the background must get out of
/// the way the moment the operator wants the machine, and "between clips" is soon
/// enough that they will not notice the difference.
pub fn run(
    scout_model: &Path,
    target_model: &Path,
    binary: &Path,
    paths: &[String],
    incumbent: Option<Decode>,
    keep_going: impl Fn() -> bool,
    mut reading: impl FnMut(&str) -> Option<crate::db::VerseRecord>,
    mut say: impl FnMut(&str, usize, usize, f32, f32),
) -> Result<Learned, String> {
    const LISTEN_SHARE: f32 = 0.6;
    let scout_decode = Decode::for_model(scout_model);

    // ---- pass one: listen to everything, and note what was read out --------------
    let mut clips: Vec<Vec<f32>> = Vec::new(); // the clips worth re-testing
    let mut truth: Vec<(usize, String, u16)> = Vec::new(); // (clip index, book, chapter)
    let mut learned: Vec<(String, String)> = Vec::new();
    let mut minutes = 0.0f32;
    let mut used = 0usize;
    let mut rooms: Vec<Room> = Vec::new();
    let mut versions: HashMap<String, usize> = HashMap::new();

    // Size the bar by total audio, not by file count: sermons differ wildly in length.
    let mut decoded: Vec<(String, Vec<(f32, Vec<f32>)>)> = Vec::new();
    for (fi, path) in paths.iter().enumerate() {
        say("Reading the recordings", fi, paths.len(), 0.0, 0.05);
        let audio = match decode_audio_file(Path::new(path)) {
            Ok(a) => a,
            Err(_) => continue, // a file we cannot read is not worth abandoning the rest for
        };
        minutes += audio.len() as f32 / 16_000.0 / 60.0;
        used += 1;
        // Measured from the recording, not assumed the same as every other church's.
        rooms.push(measure_room(&audio));
        let name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        decoded.push((name, crate::audio::segment_utterances(&audio)));
    }
    if decoded.is_empty() {
        return Err("None of those files could be read as audio.".into());
    }

    let total_utterances: usize = decoded.iter().map(|(_, u)| u.len()).sum();
    let mut heard = 0usize;

    for (name, utterances) in &decoded {
        let mut transcripts: Vec<(usize, String)> = Vec::new();
        for (i, (_at, clip)) in utterances.iter().enumerate() {
            if !keep_going() {
                return Err(CANCELLED.into());
            }
            heard += 1;
            if heard % 5 == 0 {
                say(&format!("Listening to {name}"), heard, total_utterances, 0.05, LISTEN_SHARE);
            }
            if let Ok(raw) = crate::stt::transcribe(clip, scout_model, binary, scout_decode) {
                let text = crate::corrections::correct(raw.trim());
                if !text.is_empty() {
                    transcripts.push((i, text));
                }
            }
        }

        let mut seen: Vec<(String, u16)> = Vec::new();
        for (i, text) in &transcripts {
            let Some(rec) = reading(text) else { continue };
            if seen.iter().any(|(o, c)| *o == rec.book_osis && *c == rec.chapter) {
                continue; // one passage, however long the reading
            }
            seen.push((rec.book_osis.clone(), rec.chapter));
            *versions.entry(rec.translation.clone()).or_insert(0) += 1;

            // Keep the reading and the moments before it, where the reference was
            // announced. That is all the settings have to recover.
            let first = i.saturating_sub(4);
            let base = clips.len();
            for k in first..=*i {
                if k < utterances.len() {
                    clips.push(utterances[k].1.clone());
                }
            }
            truth.push((base + (*i - first), rec.book_osis.clone(), rec.chapter));

            // A book name that came out as a word we do not know, vouched for by the
            // chapter she then read, is worth remembering for this speaker.
            for (_, said) in transcripts.iter().filter(|(j, _)| *j < *i && *i - *j <= 4) {
                if let Some(word) = learn_book_name(said, &rec.book_osis, rec.chapter) {
                    if !learned.iter().any(|(w, _)| *w == word) {
                        learned.push((word, rec.book_osis.clone()));
                    }
                }
            }
        }
    }

    if truth.is_empty() {
        return Err("No scripture was read aloud in those recordings, so there is nothing to \
                    learn from. This needs sermons where the preacher reads the verses out."
            .into());
    }

    // ---- pass two: which settings recover the most of it? ------------------------
    let baseline = Decode::for_model(target_model);
    let mut configs = candidates();
    // The settings in force get measured on this audio too, even if they are not one of
    // the usual candidates — otherwise "better than what we have" would be a guess.
    if let Some(inc) = incumbent {
        if !configs.contains(&inc) {
            configs.push(inc);
        }
    }
    let mut best: Option<(Decode, usize)> = None;
    let mut before = 0usize;
    let mut incumbent_score: Option<usize> = None;

    for (ci, cfg) in configs.iter().enumerate() {
        say(
            "Comparing recognizer settings",
            ci,
            configs.len(),
            0.05 + LISTEN_SHARE,
            1.0 - LISTEN_SHARE - 0.05,
        );
        let mut texts: HashMap<usize, String> = HashMap::new();
        for (k, clip) in clips.iter().enumerate() {
            if !keep_going() {
                return Err(CANCELLED.into());
            }
            if let Ok(raw) = crate::stt::transcribe(clip, target_model, binary, *cfg) {
                texts.insert(k, crate::corrections::correct(raw.trim()));
            }
        }
        let found = truth
            .iter()
            .filter(|(i, osis, chapter)| {
                (i.saturating_sub(4)..=*i)
                    .any(|k| texts.get(&k).map(|t| resolves_to(t, osis, *chapter)).unwrap_or(false))
            })
            .count();
        if *cfg == baseline {
            before = found;
        }
        if Some(*cfg) == incumbent {
            incumbent_score = Some(found);
        }
        if best.as_ref().map(|(_, b)| found > *b).unwrap_or(true) {
            best = Some((*cfg, found));
        }
    }

    let (decode, after) = best.ok_or("nothing to compare")?;

    // The room: the median of what each recording said, so one odd file cannot skew it.
    let room = {
        let mut levels: Vec<f32> = rooms.iter().map(|r| r.speech_above).collect();
        levels.sort_by(f32::total_cmp);
        levels.get(levels.len() / 2).map(|v| Room { speech_above: *v }).unwrap_or_default()
    };
    // The version: whichever their readings matched most often, and only if it clearly
    // won — a preacher who quotes loosely should not have the screen switched on a whim.
    let translation = versions
        .iter()
        .max_by_key(|(_, n)| **n)
        .filter(|(_, n)| **n * 2 >= truth.len().max(1))
        .map(|(code, _)| code.clone());

    say("Done", 1, 1, 1.0, 0.0);

    Ok(Learned {
        decode,
        room,
        translation,
        aliases: learned,
        references_found: truth.len(),
        before,
        after,
        incumbent: incumbent_score,
        minutes,
        recordings: used,
    })
}
