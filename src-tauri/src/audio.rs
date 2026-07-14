use crate::commands::{build_payload, AppState};
use crate::detect::{self, DetectSource, Detection, RefContext};
use crate::events::Candidate;
use crate::{semantic, stt};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::path::{Path, PathBuf};
use std::collections::VecDeque;
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
const IDLE_STOP_SECS: u64 = 20 * 60; // stop listening after 20min with nothing said
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

/// Can this model keep up with mid-utterance interim passes? They make a long
/// spoken reference land before the speaker stops, but only if a pass finishes
/// faster than speech arrives — otherwise they pile up and everything lags.
///
/// small used to be excluded: it took ~20s per pass. With the model resident and
/// the encoder window fitted it takes ~1.3s, so it now qualifies. medium does not.
fn model_is_fast(model: &Path) -> bool {
    model
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.contains("tiny") || n.contains("base") || n.contains("small"))
        .unwrap_or(false)
}

/// Trim trailing silence (keeping ~0.2s) so whisper isn't fed a second-plus of
/// dead air after the speaker stops — pure latency with no benefit.
fn trim_trailing_silence(audio: &[f32]) -> Vec<f32> {
    let win = TARGET_RATE as usize / 10; // 100 ms
    let keep = TARGET_RATE as usize / 5; // 200 ms tail
    let mut end = audio.len();
    while end >= win {
        if rms(&audio[end - win..end]) > SILENCE_RMS {
            break;
        }
        end -= win;
    }
    audio[..(end + keep).min(audio.len())].to_vec()
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
    let cleaned = joined
        .replace("[BLANK_AUDIO]", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    // Fix common speech-to-text mishearings of biblical names before detection.
    crate::corrections::correct(&cleaned)
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

/// Collect ranked verse candidates for a piece of description: famous stories
/// (keyword/phrase, scored best-first) then strong FTS quote matches. Used both
/// for the first uncertain guess and for every refinement pass.
fn research(
    db: &crate::db::Db,
    tr: &str,
    text: &str,
    min_keywords: usize,
) -> Vec<crate::reference::ParsedRef> {
    use crate::reference::ParsedRef;
    let mut refs: Vec<ParsedRef> = Vec::new();
    for h in crate::knowledge::detect_stories_scored(text, min_keywords) {
        let r = ParsedRef { book_osis: h.osis, chapter: h.chapter, verse: Some(h.verse) };
        if !refs.contains(&r) {
            refs.push(r);
        }
    }
    if let Some((query, words)) = semantic::fts_query(text) {
        // Active translation first, then any installed translation — so a verse
        // quoted from memory in a different wording (e.g. KJV while set to WEB)
        // still matches; present_best resolves the coordinates in the active text.
        let active = db.search_fts(tr, &query, 3).unwrap_or_default();
        let any = db.search_fts_any(&query, 3).unwrap_or_default();
        for (rec, _rank) in active.into_iter().chain(any) {
            if semantic::is_strong(semantic::overlap(&words, &rec.text), words.len()) {
                let r = ParsedRef { book_osis: rec.book_osis, chapter: rec.chapter, verse: Some(rec.verse) };
                if !refs.contains(&r) {
                    refs.push(r);
                }
            }
        }
    }
    refs
}

/// Append `extra` after `primary`, dropping duplicates, preserving order.
fn merge_ranked(
    mut primary: Vec<crate::reference::ParsedRef>,
    extra: Vec<crate::reference::ParsedRef>,
) -> Vec<crate::reference::ParsedRef> {
    for r in extra {
        if !primary.contains(&r) {
            primary.push(r);
        }
    }
    primary
}

/// How far to trust a verse recovered from the speaker's own words.
///
/// A flat 0.7 treated a verbatim reading and a vague allusion alike, and 0.7 sits
/// below the auto-project bar — so scripture the speaker was *reading out* stayed
/// a suggestion nobody had asked for a second opinion on. Across six recorded
/// sermons that was 7 of the 17 verses a human operator put on the screen.
///
/// So the confidence follows the evidence: when most of what he just said is in
/// the verse, he is reading it, and it goes up like any named reference. A loose
/// echo stays a suggestion.
pub fn quote_confidence(text: &str, verse_text: &str) -> f32 {
    // The question is how much of THE VERSE he said — not how much of what he said
    // is in the verse. Those are different, and the second one is wrong: an
    // utterance is ~12s of speech and usually carries a verse *plus* the next one
    // plus commentary, so a faithful reading scores poorly against any single verse.
    // Measured that way, scripture being read aloud stayed below the bar.
    let spoken: std::collections::HashSet<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4)
        .map(|w| w.to_lowercase())
        .collect();
    let key: Vec<String> = verse_text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4)
        .map(|w| w.to_lowercase())
        .filter(|w| !semantic::is_filler(w))
        .collect();
    if key.len() < 4 {
        return 0.7; // too short to judge by its words alone
    }
    let said = key.iter().filter(|w| spoken.contains(*w)).count();
    let coverage = said as f32 / key.len() as f32;
    if coverage >= 0.65 {
        0.88 // he is reading it out
    } else if coverage >= 0.45 {
        0.75 // quoting loosely
    } else {
        0.7 // alluding
    }
}

/// Emit the top standby candidate (unless it is already the last thing shown) and
/// record its text so we can notice the speaker reading it. Returns false if the
/// candidate cannot be resolved to a verse.
fn present_best(
    app: &AppHandle,
    db: &crate::db::Db,
    tr: &str,
    text: &str,
    last: &mut Option<RefKey>,
    p: &mut crate::resolution::Pending,
) -> bool {
    let Some(r) = p.candidates.first().cloned() else {
        return false;
    };
    let rec = match db.find_verse(tr, &r) {
        Ok(Some(rec)) => rec,
        _ => return false,
    };
    let payload = build_payload(rec);
    p.presented_text = payload.text.clone();
    let confidence = quote_confidence(text, &payload.text);
    let source = if confidence >= 0.88 { "quote" } else { "suggestion" };
    let key: RefKey = (r.book_osis.clone(), r.chapter, r.verse);
    if last.as_ref() != Some(&key) {
        let _ = app.emit(
            "verse-candidate",
            Candidate { verse: payload, confidence, source: source.into() },
        );
        *last = Some(key);
    }
    // Surface the standby alternatives so the operator can pick one directly.
    let alternatives: Vec<crate::events::VersePayload> = p
        .candidates
        .iter()
        .skip(1)
        .take(4)
        .filter_map(|alt| db.find_verse(tr, alt).ok().flatten().map(build_payload))
        .collect();
    let _ = app.emit("verse-alternatives", alternatives);
    true
}

/// Consecutive non-refining utterances after which we stop actively looping and
/// let the last suggestion stand (the speaker has moved on).
const REFINE_GIVE_UP: u32 = 2;

/// The speaker is reading aloud from the passage already on screen — which verse
/// of it? Searches only within the presented book and chapter, so this can sharpen
/// the projection but never move it somewhere else. Returns None unless the words
/// match a verse strongly enough to be a reading rather than a passing allusion.
fn read_along(
    db: &crate::db::Db,
    tr: &str,
    text: &str,
    last: &Option<RefKey>,
) -> Option<crate::db::VerseRecord> {
    let (book, chapter, _) = last.as_ref()?;
    let (query, words) = semantic::fts_query(text)?;
    let hits = db.search_fts(tr, &query, 8).ok()?;
    hits.into_iter().map(|(rec, _)| rec).find(|rec| {
        rec.book_osis == *book
            && rec.chapter == *chapter
            && semantic::is_strong(semantic::overlap(&words, &rec.text), words.len())
    })
}

#[cfg(test)]
mod tests {
    /// Verses recovered from the speaker's own words, graded by how much of the
    /// verse he actually said. Reading it out earns the same trust as naming it;
    /// an allusion does not. (0.82 is the default auto-project bar.)
    #[test]
    fn reading_a_verse_aloud_is_trusted_more_than_alluding_to_it() {
        use super::quote_confidence;
        let verse = "Ye are the light of the world. A city that is set on an hill cannot be hid.";

        let reading = "you are the light of the world a city set on the hill cannot be hidden";
        assert!(quote_confidence(reading, verse) >= 0.82, "a reading should project");

        let allusion = "we are supposed to be light in this dark generation somehow";
        assert!(quote_confidence(allusion, verse) < 0.82, "an allusion should only suggest");
    }

    use super::merge_ranked;
    use crate::reference::ParsedRef;

    fn r(osis: &str, ch: u16, v: u16) -> ParsedRef {
        ParsedRef { book_osis: osis.into(), chapter: ch, verse: Some(v) }
    }

    #[test]
    fn merge_ranked_keeps_primary_first_and_dedups() {
        // Fresh search results rank first; remaining standby items follow;
        // duplicates are dropped, order preserved.
        let fresh = vec![r("Luke", 15, 3), r("Luke", 15, 11)];
        let standby = vec![r("Luke", 15, 11), r("Matt", 18, 12)];
        let merged = merge_ranked(fresh, standby);
        assert_eq!(
            merged,
            vec![r("Luke", 15, 3), r("Luke", 15, 11), r("Matt", 18, 12)]
        );
    }
}

/// Transcribe a clip, emit transcript (final only), and emit new candidates.
/// Shared by the final (endpointed) flush and the interim mid-utterance pass.
#[allow(clippy::too_many_arguments)] // threads per-loop mic/detection state
fn transcribe_detect(
    app: &AppHandle,
    model: &Path,
    binary: &Path,
    decode: stt::Decode,
    audio: &[f32],
    ctx: &mut RefContext,
    last: &mut Option<RefKey>,
    pending: &mut Option<crate::resolution::Pending>,
    recent: &mut VecDeque<String>,
    emit_transcript: bool,
) {
    let raw = match stt::transcribe(audio, model, binary, decode) {
        Ok(t) => t,
        Err(e) => {
            let _ = app.emit("listen-error", e);
            return;
        }
    };
    let text = clean_transcript(&raw);
    // Capture the endpointed utterance (audio + what the model heard) when
    // calibration is on, so decode settings can be tuned against real speech.
    if emit_transcript {
        crate::capture::save(app, audio, &raw, &text, model);
    }
    if text.is_empty() {
        return;
    }
    if emit_transcript {
        let _ = app.emit("transcript", text.clone());
        // Keep a rolling window of recent words so a quotation spread across
        // pauses can still be recognized as a whole.
        for w in text.split_whitespace() {
            recent.push_back(w.to_lowercase());
        }
        while recent.len() > 40 {
            recent.pop_front();
        }
    }

    let detections = detect::detect_with_context(&text, ctx);
    let has_explicit = detections.iter().any(|d| d.source != DetectSource::Story);

    // Relative voice navigation ("next verse", "next chapter") wins over
    // everything and ends any pending confirmation loop.
    if detections.is_empty() {
        if let Some(dir) = detect::detect_nav_command(&text) {
            if let Some(payload) = crate::commands::navigate_handle(app, dir) {
                *pending = None;
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
    let prev_tr = state.active_translation();
    let tr = crate::commands::resolve_translation(&state, &text);
    // If the translation just changed and this utterance names no new reference
    // (e.g. "...now give it to me in the NIV" said just after the verse),
    // re-project the current scripture in the new translation.
    if tr != prev_tr && detections.is_empty() {
        if let Some((osis, ch, v)) = state.cursor.lock().ok().and_then(|c| c.as_ref().map(|c| (c.book_osis.clone(), c.chapter, c.verse))) {
            crate::commands::present_coords_handle(app, &osis, ch, v);
        }
    }

    // --- Confirm/refine conversation loop (final utterances only) ---
    if emit_transcript {
        if let Some(p) = pending.take() {
            if !has_explicit {
                let db = match state.db.lock() {
                    Ok(d) => d,
                    Err(_) => return,
                };
                match crate::resolution::classify(&text, &p.presented_text) {
                    crate::resolution::Response::Affirm => {
                        if let Some(r) = p.candidates.first() {
                            if let Ok(Some(rec)) = db.find_verse(&tr, r) {
                                let _ = app.emit("verse-confirmed", build_payload(rec));
                            }
                        }
                        return; // pending cleared
                    }
                    crate::resolution::Response::Deny => {
                        let mut p = p;
                        if !p.candidates.is_empty() {
                            p.candidates.remove(0);
                        }
                        let desc = crate::resolution::description_part(&text);
                        if !desc.is_empty() {
                            p.description.push(' ');
                            p.description.push_str(&desc);
                        }
                        let found = research(&db, &tr, &p.description, 2);
                        p.candidates = merge_ranked(found, std::mem::take(&mut p.candidates));
                        p.misses = 0;
                        if present_best(app, &db, &tr, &text, last, &mut p) {
                            *pending = Some(p);
                        }
                        return;
                    }
                    crate::resolution::Response::Other => {
                        let mut p = p;
                        p.description.push(' ');
                        p.description.push_str(&text);
                        let found = research(&db, &tr, &p.description, 2);
                        let improved = matches!(
                            (found.first(), p.candidates.first()),
                            (Some(nb), cur) if Some(nb) != cur
                        );
                        if improved {
                            p.candidates = merge_ranked(found, std::mem::take(&mut p.candidates));
                            p.misses = 0;
                            if present_best(app, &db, &tr, &text, last, &mut p) {
                                *pending = Some(p);
                            }
                        } else {
                            p.misses += 1;
                            if p.misses < REFINE_GIVE_UP {
                                *pending = Some(p); // keep waiting a little longer
                            }
                            // else: settle — the last suggestion stands.
                        }
                        return;
                    }
                }
            }
            // has_explicit: a fresh reference supersedes the loop; fall through.
        }
    }

    let _ = app.emit("translation-changed", &tr);
    let db = match state.db.lock() {
        Ok(d) => d,
        Err(_) => return,
    };
    // Did a confident reference actually reach the screen? If none of them resolve
    // to real scripture, the utterance is *not* handled — fall through and let the
    // quote/story matcher have it.
    let mut projected = false;
    if has_explicit {
        // Confident references: explicit / fuzzy / context / descriptive.
        //
        // Only the LAST one is projected. Preachers correct themselves mid-sentence
        // — "when you read 2 Chronicles, 1 Chronicles chapter 20, verse 22" — and
        // projecting every hit in turn threw three different verses on the wall in
        // a second, two of them wrong. The final reference in an utterance is the
        // one the speaker settled on; the earlier ones are offered as alternatives
        // so nothing is lost if the guess is wrong.
        let confident: Vec<&Detection> =
            detections.iter().filter(|d| d.source != DetectSource::Story).collect();
        for d in confident.iter().rev().take(1) {
            let r = &d.reference;
            let key: RefKey = (r.book_osis.clone(), r.chapter, r.verse);
            if last.as_ref() == Some(&key) {
                continue;
            }
            // A bare chapter mention must not knock a verse of that same chapter off
            // the screen. A preacher working through Romans 8:18 says "this chapter,
            // Romans 8" constantly, and every one of those was resetting the wall to
            // Romans 8:1 — a verse nobody asked for. She means the chapter she is
            // already in. A bare chapter of a *different* passage is a real move, and
            // still projects.
            let bare_chapter_of_what_is_showing = r.verse.is_none()
                && last
                    .as_ref()
                    .map(|(b, c, v)| *b == r.book_osis && *c == r.chapter && v.is_some())
                    .unwrap_or(false);
            if bare_chapter_of_what_is_showing {
                continue;
            }
            if let Ok(Some(rec)) = db.find_verse(&tr, r) {
                let (confidence, source) = confidence_of(d);
                // Spoken verse range ("John 3:16 through 18", "the whole chapter"):
                // resolve an open-ended span to the chapter's last verse.
                let payload = match (r.verse, d.verse_end) {
                    (Some(start), Some(end)) => {
                        let real_end = if end == detect::TO_CHAPTER_END {
                            db.chapter_last_verse(&tr, &r.book_osis, r.chapter)
                                .ok()
                                .flatten()
                                .unwrap_or(start)
                        } else {
                            end
                        };
                        crate::commands::build_range_payload(
                            &db, &tr, &r.book_osis, r.chapter, start, real_end,
                        )
                        .unwrap_or_else(|| build_payload(rec))
                    }
                    _ => build_payload(rec),
                };
                let _ = app.emit(
                    "verse-candidate",
                    Candidate { verse: payload, confidence, source: source.to_string() },
                );
                *last = Some(key);
                projected = true;

                // The references the speaker passed through on the way here — the
                // "2 Chronicles" of a "2 Chronicles, 1 Chronicles" correction — so
                // the operator can still reach them in one click if we chose wrong.
                let others: Vec<crate::events::VersePayload> = confident
                    .iter()
                    .rev()
                    .skip(1)
                    .filter_map(|o| db.find_verse(&tr, &o.reference).ok().flatten().map(build_payload))
                    .take(4)
                    .collect();
                let _ = app.emit("verse-alternatives", others);
            }
        }
        // The reference does not exist — "Colossians chapter 22", built from a
        // book remembered minutes ago. The memory is evidently stale, so drop it
        // rather than keep mapping loose verse numbers onto the wrong book. This
        // was suppressing real hits: a preacher quoting "sit on thrones judging
        // the twelve tribes of Israel" and saying "verse 29" produced an
        // impossible Colossians 22:29 and nothing else, when the words themselves
        // were Luke 22:30 and the quote matcher could have found them.
        if !projected {
            ctx.clear();
        }
    }

    // Having named the reference, the preacher reads it out. Those words identify
    // the verse by themselves — no need to have heard the name right — so they are
    // the strongest confirmation available, and the app was throwing them away.
    //
    // Used to sharpen what is already on the wall: a chapter-only reference
    // ("Matthew chapter 5") lands on the verse actually being read ("blessed are
    // the poor in spirit" → 5:3), and a misheard verse number corrects itself. The
    // match is confined to the book and chapter already showing, so reading a verse
    // aloud can never yank the screen to another book.
    if !projected {
        if let Some(refined) = read_along(&db, &tr, &text, last) {
            let payload = build_payload(refined);
            let key: RefKey = (payload.book_osis.clone(), payload.chapter, Some(payload.verse));
            if last.as_ref() != Some(&key) {
                let _ = app.emit(
                    "verse-candidate",
                    Candidate { verse: payload, confidence: 0.88, source: "read-along".into() },
                );
                *last = Some(key);
                projected = true;
            }
        }
    }

    if !projected {
        // Uncertain: a topical request ("what the Bible says about worry"), a
        // paraphrased story, or a quoted verse. Present the best guess and, on a
        // final utterance, open the confirm/refine loop with the alternatives on
        // standby (for a topic, that's the whole set of verses to cycle through).
        let topic = crate::knowledge::detect_topic(&text);
        let ranked: Vec<crate::reference::ParsedRef> = match &topic {
            Some((_name, refs)) => refs
                .iter()
                .map(|(o, c, v)| crate::reference::ParsedRef {
                    book_osis: o.clone(),
                    chapter: *c,
                    verse: Some(*v),
                })
                .collect(),
            None => research(&db, &tr, &text, 3),
        };
        // Nothing from this utterance alone — try the rolling window in case a
        // quotation was spoken across a pause.
        let mut ranked = if ranked.is_empty() && emit_transcript && recent.len() >= 8 {
            let joined: String = recent.iter().cloned().collect::<Vec<_>>().join(" ");
            research(&db, &tr, &joined, 3)
        } else {
            ranked
        };
        // Operator learning: if this description was corrected before, rank the
        // remembered verse first.
        if let Ok(learned) = state.learned.lock() {
            if let Some((o, c, v)) = learned.get(&crate::resolution::signature(&text)) {
                let chosen = crate::reference::ParsedRef { book_osis: o.clone(), chapter: *c, verse: Some(*v) };
                ranked.retain(|r| r != &chosen);
                ranked.insert(0, chosen);
            }
        }
        if !ranked.is_empty() {
            if let Some((name, _)) = &topic {
                let _ = app.emit("topic-detected", name.clone());
            }
            let mut p = crate::resolution::Pending {
                candidates: ranked,
                description: text.clone(),
                presented_text: String::new(),
                misses: 0,
            };
            if present_best(app, &db, &tr, &text, last, &mut p) && emit_transcript {
                *pending = Some(p);
            }
        }
    }
}

/// Cut a recording into utterances the way the live loop cuts the microphone:
/// same silence threshold, same endpoint delay, same minimum speech, same cap.
/// Returns each utterance with the second it began, so a replay of a recorded
/// sermon exercises the real path rather than an idealized one.
///
/// The streaming loop in `run_inner` is the twin of this; they must agree.
pub fn segment_utterances(samples: &[f32]) -> Vec<(f32, Vec<f32>)> {
    const FRAME: usize = (TARGET_RATE as usize) / 100; // 10 ms, as the mic delivers
    let mut out = Vec::new();
    let mut utter: Vec<f32> = Vec::new();
    let mut preroll: Vec<f32> = Vec::new();
    let mut speech_ms = 0f32;
    let mut silence_ms = 0f32;
    let mut start_sample = 0usize;

    for (i, frame) in samples.chunks(FRAME).enumerate() {
        let at = i * FRAME;
        let dur = ms_of(frame.len());
        if rms(frame) > SILENCE_RMS {
            if utter.is_empty() {
                start_sample = at.saturating_sub(preroll.len());
                utter.extend_from_slice(&preroll);
            }
            utter.extend_from_slice(frame);
            speech_ms += dur;
            silence_ms = 0.0;
        } else if utter.is_empty() {
            preroll.extend_from_slice(frame);
            if preroll.len() > PREROLL_SAMPLES {
                let drop_n = preroll.len() - PREROLL_SAMPLES;
                preroll.drain(0..drop_n);
            }
        } else {
            utter.extend_from_slice(frame);
            silence_ms += dur;
        }

        let ended = (silence_ms >= SILENCE_FLUSH_MS && !utter.is_empty())
            || ms_of(utter.len()) >= MAX_UTTER_MS;
        if ended {
            if speech_ms >= MIN_SPEECH_MS {
                out.push((
                    start_sample as f32 / TARGET_RATE as f32,
                    trim_trailing_silence(&utter),
                ));
            }
            utter.clear();
            preroll.clear();
            speech_ms = 0.0;
            silence_ms = 0.0;
        }
    }
    if speech_ms >= MIN_SPEECH_MS && !utter.is_empty() {
        out.push((start_sample as f32 / TARGET_RATE as f32, trim_trailing_silence(&utter)));
    }
    out
}

/// Open the default microphone and stream 16 kHz mono frames down a channel.
/// Shared by the listen loop and the calibration recorder, so both hear exactly
/// the same audio path (same device, same resampling, same levels).
fn open_mic() -> Result<(cpal::Stream, mpsc::Receiver<Vec<f32>>), String> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("no input microphone found")?;
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let channels = config.channels as usize;
    let src_rate = config.sample_rate.0 as f32;

    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let err_fn = |e| eprintln!("audio stream error: {e}");

    let stream = match sample_format {
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
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, rx))
}

/// Record exactly one spoken utterance: wait for speech, then endpoint on
/// silence. Used by calibration, where each clip must line up with one prompt.
/// Returns None if nobody spoke before `wait_secs`.
pub fn record_one_utterance(wait_secs: u64) -> Result<Option<Vec<f32>>, String> {
    let (stream, rx) = open_mic()?;
    let mut utter: Vec<f32> = Vec::new();
    let mut preroll: Vec<f32> = Vec::new();
    let mut speech_ms = 0f32;
    let mut silence_ms = 0f32;
    let deadline = Instant::now() + Duration::from_secs(wait_secs);

    loop {
        if utter.is_empty() && Instant::now() > deadline {
            drop(stream);
            return Ok(None);
        }
        match rx.recv_timeout(Duration::from_millis(150)) {
            Ok(frame) => {
                let dur = ms_of(frame.len());
                if rms(&frame) > SILENCE_RMS {
                    if utter.is_empty() {
                        utter.extend_from_slice(&preroll);
                    }
                    utter.extend_from_slice(&frame);
                    speech_ms += dur;
                    silence_ms = 0.0;
                } else if utter.is_empty() {
                    preroll.extend_from_slice(&frame);
                    if preroll.len() > PREROLL_SAMPLES {
                        let drop_n = preroll.len() - PREROLL_SAMPLES;
                        preroll.drain(0..drop_n);
                    }
                } else {
                    utter.extend_from_slice(&frame);
                    silence_ms += dur;
                }

                let done = (silence_ms >= SILENCE_FLUSH_MS && speech_ms >= MIN_SPEECH_MS)
                    || ms_of(utter.len()) >= MAX_UTTER_MS;
                if done {
                    drop(stream);
                    return Ok(Some(trim_trailing_silence(&utter)));
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                drop(stream);
                return Err("microphone stream ended".into());
            }
        }
    }
}

fn run_inner(
    app: &AppHandle,
    flag: &Arc<AtomicBool>,
    model: &Path,
    binary: &Path,
    decode: stt::Decode,
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
    let mut pending: Option<crate::resolution::Pending> = None;
    let mut recent_words: VecDeque<String> = VecDeque::new();
    let mut last_activity = Instant::now();
    let interim_enabled = model_is_fast(model);

    while flag.load(Ordering::SeqCst) {
        // A listening session left running transcribes room noise indefinitely,
        // which is pure CPU burn nobody asked for. Stand down after a long
        // silence; the operator can start it again in one click.
        if last_activity.elapsed().as_secs() > IDLE_STOP_SECS {
            let _ = app.emit("listen-idle-stop", ());
            break;
        }
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

                // Interim pass: only for fast models on long continuous speech
                // (slow models fall behind, so they wait for the endpoint).
                if interim_enabled
                    && speech_ms >= MIN_SPEECH_MS
                    && utter.len().saturating_sub(last_interim) >= INTERIM_SAMPLES
                {
                    transcribe_detect(app, model, binary, decode, &utter, &mut ctx, &mut last_ref, &mut pending, &mut recent_words, false);
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
                        let audio = trim_trailing_silence(&audio);
                        transcribe_detect(app, model, binary, decode, &audio, &mut ctx, &mut last_ref, &mut pending, &mut recent_words, true);
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
                            let audio = trim_trailing_silence(&audio);
                            transcribe_detect(app, model, binary, decode, &audio, &mut ctx, &mut last_ref, &mut pending, &mut recent_words, true);
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

pub fn run_listen_loop(
    app: AppHandle,
    flag: Arc<AtomicBool>,
    model: PathBuf,
    binary: PathBuf,
    decode: stt::Decode,
) {
    // The env override still wins, for benching.
    let decode = match std::env::var("BIBLE_APP_DECODE") {
        Ok(_) => stt::Decode::from_env_or_model(&model),
        Err(_) => decode,
    };
    if let Err(e) = run_inner(&app, &flag, &model, &binary, decode) {
        let _ = app.emit("listen-error", e);
    }
    flag.store(false, Ordering::SeqCst);
    // The whisper server holds the model in RAM; it must not outlive the session.
    stt::shutdown();
    let _ = app.emit("listen-stopped", ());
}
