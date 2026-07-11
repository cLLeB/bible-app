# Offline Church Bible App — Design Spec

**Date:** 2026-07-11
**Status:** Approved (design) → ready for implementation planning
**Location:** `C:\Users\kyere\Documents\codes\bible-app`

---

## 1. Mission & Differentiator

A church presentation app that **runs 100% offline after installation** on an average/old Windows laptop — no cloud, no subscriptions, no per-seat licensing. It competes with RhemaCast and EasyVerse not on feature count but on the one thing neither does well: **fully offline live transcription + verse detection**, with low hardware requirements.

The bet: the hard problem here is a *constrained retrieval* problem (find one of ~31,102 verses), not a general-AI problem. That is tractable offline. Everything else (templates, remotes, plugins) is incremental polish added later.

## 2. Core Features (MVP)

1. **Live-mic offline speech-to-text.**
2. **Automatic Bible reference detection**, including **quoted/paraphrased** verse detection.
3. **Song lyrics** with automatic slide splitting.
4. **Projection** to a second monitor.

Bundled offline Bible database with multiple public-domain translations.

## 3. Non-Goals (explicitly deferred)

Stage display, QR/Wi-Fi remotes, NDI, OBS/vMix output, ProPresenter/OpenLP/PowerPoint plugins, cloud sync, theme/timeline editors, playlists. These are Phase 5+ and must not add storage/RAM cost to the core.

## 4. Locked Technical Decisions

| Concern | Decision | Rationale |
|---|---|---|
| Shell / runtime | **Tauri 2** (Rust core) + **React + TS + Vite + Tailwind + Zustand** | Tiny installer (~<20MB pre-models), low idle RAM, native perf on old laptops. |
| Speech-to-text | **whisper.cpp** via `whisper-rs`, GGUF models `tiny.en`/`base.en`/`small.en` (user-selectable) | CPU-optimized, no GPU, quantized, runs on old hardware, fully local. |
| Voice activity | **Silero VAD** (ONNX) | Only transcribe/query on speech; cheap gating. |
| Embeddings | **ONNX Runtime in Rust** (`ort`) running `all-MiniLM-L6-v2` (~80MB) | Runs on-device for BOTH live-transcript queries AND indexing user-imported translations. Python is **build-time only**; end users need no Python. |
| Vector search | **Brute-force cosine** over a preloaded in-memory `f32` matrix | 31,102 × 384 = ~48MB/translation; sub-ms scan. No ANN lib, no `sqlite-vec`, no index rebuilds. The corpus is tiny and fixed — the simple approach is the correct one. |
| Database | **SQLite** via `rusqlite` (bundled), **FTS5** for lexical song/verse search | Single-file, offline, zero-config. |
| Bible text | Bundle public-domain **KJV + WEB + ASV**; support **optional user import** of licensed translations (NIV/ESV/NLT/NKJV) the church legally owns, indexed on-device after import | Legally distributable core; churches extend to their preferred translation. |

## 5. Architecture

### 5.1 Module map

```
        React UI (operator console)   +   Projection Window (2nd monitor)
                       ▲   │  Tauri IPC (commands ↑ / events ↓)
                       │   ▼
┌──────────────────── Rust core ─────────────────────────────────────────┐
│                                                                         │
│  audio     cpal capture + Silero VAD (ONNX) → 16kHz mono speech frames  │
│    │  event: SpeechSegment                                              │
│    ▼                                                                    │
│  stt       whisper.cpp (whisper-rs) → rolling transcript + timestamps   │
│    │  event: TranscriptChunk                                            │
│    ▼                                                                    │
│  detect    L1 regex refs                                                │
│            L2 book alias + fuzzy (strsim)                               │
│            L3 NL cue phrases ("turn with me to…", "reading from…")      │
│            L4 semantic: embed chunk (ONNX MiniLM) → cosine vs matrix    │
│    │  emits: Vec<VerseCandidate>                                        │
│    ▼                                                                    │
│  resolver  merge/dedupe across layers · confidence thresholds ·         │
│            debounce/cooldown · staging queue · accept policy            │
│            (staging manual | auto-advance ≥ threshold) · history        │
│    │  command → projection ;  event → UI (suggestions rail)             │
│    ▼                                                                    │
│  projection  2nd-monitor window state: verse | song-slide | blank | logo│
│                                                                         │
│  library   SQLite: bibles, verses, embeddings(blob), songs, aliases,    │
│            settings, history · FTS5 lexical search                      │
│  importer  parse translation file → embed on-device → write index       │
└─────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Module contracts (one purpose each)

- **audio** — captures mic, gates on VAD, emits `SpeechSegment{pcm, t0, t1}`. Depends on: cpal, Silero ONNX. Testable with recorded WAV fixtures.
- **stt** — turns a segment into `TranscriptChunk{text, words[], t0, t1}`. Depends on: whisper-rs, model file. Testable with fixed audio → expected text (fuzzy).
- **detect** — pure function-ish: `TranscriptChunk → Vec<VerseCandidate>`. `VerseCandidate{ref, translation, score, source_layer}`. Depends on: alias table, embedding matrix, MiniLM. Fully unit-testable from text (no audio).
- **resolver** — stateful policy engine: `Vec<VerseCandidate> → StagingQueue + ProjectionCommand?`. Owns thresholds, debounce, accept mode. Depends on: nothing external. Unit-testable with candidate streams.
- **projection** — renders a `ProjectionState`. Depends on: Tauri window on 2nd display. Testable via state assertions.
- **library** — repository over SQLite (findVerse, searchSongs, loadEmbeddings, history). Depends on: rusqlite. Testable against a temp DB.
- **importer** — `translation file → validated verses → embeddings → persisted index`. Depends on: library, MiniLM. Testable with a small sample file.

### 5.3 Data model (SQLite sketch)

```
translations(id, code, name, license, is_public_domain, is_bundled)
books(id, translation_id, number, name, osis, testament)
verses(id, translation_id, book_id, chapter, verse, text)
verse_embeddings(verse_id, translation_id, vector BLOB)      -- 384×f32
verses_fts(text, verse_id)                                   -- FTS5
songs(id, title, author, ccli, lyrics)
song_slides(id, song_id, order_index, text)
aliases(alias, book_osis, weight)                            -- "jn","roots"→Ruth, etc.
settings(key, value)
history(id, ts, kind, ref_or_song, translation_id, source_layer, score)
```

At startup, `verse_embeddings` for loaded translations are read once into an in-memory `{translation_id → (Vec<VerseId>, Vec<f32> matrix)}` for brute-force cosine.

## 6. Detection Pipeline (the differentiator)

- **L1 Regex** — `John 3:16`, `Romans 8`, `1 Cor 13`, `First Corinthians 13`, `Genesis chapter 2`. ~70% of explicit references.
- **L2 Alias + fuzzy** — thousands of book aliases + `strsim` recovery for STT errors (`roots`→Ruth, `hosier`→Hosea). Includes a **Bible-aware post-processor** that corrects likely misheard book names before parsing.
- **L3 NL cues** — trigger phrases ("turn with me to", "open your Bible to", "reading from", "beginning at verse") that mark an incoming reference and bias parsing.
- **L4 Semantic** — embed the transcript window with MiniLM, brute-force cosine against the preloaded matrix, return top-k verses with scores. This is what catches *"The Lord is my shepherd"* → Psalm 23:1 with no explicit reference.

`detect` returns all candidates with `source_layer` and `score`; the **resolver** decides what (if anything) reaches the operator/projection.

## 7. Control Model

**Staging-first.** Candidates appear in a suggestions rail with confidence; operator accepts (Enter/click) to project. Wrong guesses never reach the wall. **Auto-advance** is a later per-confidence-threshold toggle layered on the same pipeline (`resolver` auto-accepts above score X). Same detection code; auto-advance ≈ a 1-day addition once confidence numbers are trusted.

## 8. Roadmap (vertical slices)

- **Phase 1 — Shell + projection seam (no ML).** Tauri app, SQLite loaded with WEB, manual reference lookup (`John 3:16` → renders on 2nd monitor), song entry + slide render. Proves DB schema, IPC, and the "put this on the wall" seam every later phase plugs into.
- **Phase 2 — Live STT + explicit detection.** whisper.cpp integration, mic capture + VAD, rolling transcript, L1–L3 detection, resolver staging queue, operator-confirmed projection.
- **Phase 3 — Semantic / paraphrase.** Precompute + load embedding matrix, L4 cosine search, ranked-confidence paraphrase suggestions, optional auto-advance toggle.
- **Phase 4 — Song library polish + import.** Auto slide splitting, FTS search, translation import pipeline (on-device indexing), keyboard-driven service workflow, "church learning mode" (persist operator corrections like `Jaira→Jireh`).
- **Phase 5 (optional).** OBS browser source, NDI, Wi-Fi remote, plugin SDK. Only if zero cost to core.

## 9. Skills Required

- **Rust:** ownership, `tokio` async, channels/threads, `serde`, `rusqlite`, error handling, FFI to whisper.cpp.
- **AI/runtime:** whisper.cpp/GGUF/quantization, ONNX Runtime (`ort`), embeddings + cosine similarity.
- **Audio:** mic capture (`cpal`), VAD, streaming buffers.
- **NLP:** regex, tokenization, fuzzy matching (`strsim`), NER-lite for references.
- **Data:** SQLite, FTS5, in-memory vector scan.
- **UI:** React, Tailwind, Zustand, Tauri IPC, hotkeys, multi-window/second-screen.

## 10. Risks & Mitigations

- **STT accuracy in noisy rooms** — the one area cloud (Deepgram) leads. Mitigate with VAD, model choice, and the L2 Bible-aware corrector so book-name errors self-heal.
- **whisper.cpp real-time on old CPU** — tune chunk size + model tier; default `base.en`, allow `tiny.en`.
- **ORT model packaging on Windows** — verify `ort` static/dynamic linking early (Phase 3 spike).
- **Translation-import legality** — app never distributes licensed text; user supplies their own legal copy.

## 11. Testing Strategy

- Unit: `detect` (text→candidates) and `resolver` (candidate stream→projection decisions) are pure enough for extensive table-driven tests — the highest-value coverage.
- Integration: `library` against a temp SQLite; `importer` against a sample translation file.
- Fixture-based: `stt` against recorded WAV → expected text (fuzzy); `audio` against WAV with/without speech.
- E2E: Phase-1 manual-lookup → projection happy path.
