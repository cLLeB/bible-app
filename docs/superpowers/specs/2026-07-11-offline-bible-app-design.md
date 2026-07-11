# Offline Church Bible App — Design Spec

**Date:** 2026-07-11
**Status:** 🧊 **FROZEN (v2)** — implementation phase
**Location:** `C:\Users\kyere\Documents\codes\bible-app`

> **Change control:** This architecture is frozen. No new features are added to this document unless they fix a *proven* architectural gap discovered during implementation. All new ideas go to `docs/BACKLOG.md`. From here on: build the vertical slices, validate assumptions with working code, and measure each component on target hardware.

---

## 1. Mission & Differentiator

A church presentation app that **runs 100% offline after installation** on an average/old Windows laptop — no cloud, no subscriptions, no per-seat licensing. It competes with RhemaCast and EasyVerse not on feature count but on the one thing neither does well: **fully offline live transcription + verse detection**, with low hardware requirements.

The bet: the hard problem here is a *constrained retrieval* problem (find one of ~31,102 verses), not a general-AI problem. That is tractable offline.

**The real moat is domain specialization.** The whole chain is Bible-specific, and no general-purpose commercial recognizer is:

```
Speech → Bible-aware STT correction → Bible-aware parser → semantic search → church learning
```

A general speech engine is optimized for everything; ours is optimized for exactly one domain. That specialization is where we can rival — and in paraphrase retrieval, surpass — commercial systems even when raw STT is not always as strong. Everything else (templates, remotes, plugins) is incremental polish added later.

## 2. Core Features (MVP)

1. **Live-mic offline speech-to-text.**
2. **Automatic Bible reference detection**, including **quoted/paraphrased** verse detection.
3. **Song lyrics** with automatic slide splitting.
4. **Projection** to a second monitor.

Bundled offline Bible database with multiple public-domain translations.

## 3. Non-Goals (explicitly deferred)

Stage display, QR/Wi-Fi remotes, NDI, OBS/vMix output, ProPresenter/OpenLP/PowerPoint plugins, cloud sync, theme/timeline editors, playlists. These are Phase 5+ and must not add storage/RAM cost to the core.

**Deliberately declined for now (YAGNI):** a general publish/subscribe event *bus* (see §5.4), and a full plugin SDK with speculative traits (see §5.6). We keep the *seams* clean so these are cheap to add when a second implementation exists to shape them — but we do not build the abstractions before then.

## 4. Locked Technical Decisions

| Concern | Decision | Rationale |
|---|---|---|
| Shell / runtime | **Tauri 2** (Rust core) + **React + TS + Vite + Tailwind + Zustand** | Tiny installer (~<20MB pre-models), low idle RAM, native perf on old laptops. |
| Speech-to-text | **whisper.cpp** via `whisper-rs`, GGUF models `tiny.en`/`base.en`/`small.en` (user-selectable) | CPU-optimized, no GPU, quantized, runs on old hardware, fully local. |
| Voice activity | **Silero VAD** (ONNX) | Only transcribe/query on speech; cheap gating. |
| Embeddings | **ONNX Runtime in Rust** (`ort`) running `all-MiniLM-L6-v2` (~80MB) | Runs on-device for BOTH live-transcript queries AND indexing user-imported translations. Python is **build-time only**; end users need no Python. |
| Vector search | **Brute-force cosine** over a preloaded in-memory `f32` matrix, then lexical rerank | 31,102 × 384 = ~48MB/translation; sub-ms scan. No ANN lib, no `sqlite-vec`. The corpus is tiny and fixed — the simple approach is the correct one. |
| Database | **SQLite** via `rusqlite` (bundled), **FTS5** for lexical song/verse search + reranking | Single-file, offline, zero-config. |
| Bible text | Bundle public-domain **KJV + WEB + ASV**; support **optional user import** of licensed translations (NIV/ESV/NLT/NKJV) the church legally owns, indexed on-device after import | Legally distributable core; churches extend to their preferred translation. |
| Inter-module transport | **`tokio::mpsc` channels** carrying explicit typed events (`core/events.rs`) | Pipeline is linear; channels give backpressure without bus machinery. Upgradeable to fan-out later. |

## 5. Architecture

### 5.1 Module map

```
        React UI (operator console)   +   Projection Window (2nd monitor)
                       ▲   │  Tauri IPC (commands ↑ / events ↓)
                       │   ▼
┌──────────────────── Rust core ─────────────────────────────────────────────┐
│  pipeline    orchestration only: wires modules via channels; no AI/DB logic │
│  events      all typed event definitions (SpeechSegment … ProjectionChanged)│
│  config      runtime-tunable values (model, VAD, thresholds, debounce, keys)│
│  diagnostics latency/throughput/memory counters + structured logging        │
│                                                                             │
│  audio     cpal capture + Silero VAD (ONNX) → 16kHz mono speech frames      │
│    │  event: SpeechSegment                                                  │
│    ▼                                                                        │
│  stt       whisper.cpp (whisper-rs) → rolling transcript + timestamps       │
│    │  event: TranscriptChunk                                                │
│    ▼                                                                        │
│  detect    Detector trait, 4 impls:                                         │
│            L1 regex refs · L2 alias+fuzzy (alias_engine) ·                  │
│            L3 NL cue phrases · L4 semantic (MiniLM→cosine top20→rerank top3)│
│    │  emits: Vec<VerseCandidate{ ref, translation, Confidence, layer }>     │
│    ▼                                                                        │
│  resolver  merge/dedupe · combine Confidence · debounce/cooldown ·          │
│            staging queue · accept policy (staging | auto-advance≥τ) ·        │
│            captures operator corrections as replayable events               │
│    │  command → projection ;  event → UI (suggestions rail)                 │
│    ▼                                                                        │
│  projection  ProjectionState machine (Blank|Logo|Verse|Song|SplitScreen|Blackout)│
│                                                                             │
│  alias_engine  merges canonical_books + regional + phonetic +               │
│                common_STT_errors + church_custom_aliases → lookup           │
│  library     SQLite repo: bibles, verses, embeddings(blob), songs, aliases, │
│              settings, session-events · FTS5 search                         │
│  importer    parse translation file → embed on-device → write index         │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Module contracts (one purpose each)

- **pipeline** — owns the wiring: spawns modules, connects channels, no domain logic. Testable by feeding a scripted event stream and asserting outputs.
- **events** — plain data definitions for every message crossing a module boundary. No behavior.
- **config** — loads defaults + persisted overrides at startup; single source of truth for tunables (Whisper model, VAD sensitivity, semantic threshold τ, debounce window, auto-advance on/off, hotkeys, active translations). Hot-reloadable where cheap.
- **diagnostics** — records per-stage latency (VAD/STT/detect/projection), memory, dropped frames; structured logs. Cross-cutting; observers subscribe to the same events.
- **audio** — mic → VAD-gated `SpeechSegment{pcm, t0, t1}`. Deps: cpal, Silero ONNX. Testable with WAV fixtures.
- **stt** — `SpeechSegment → TranscriptChunk{text, words[], t0, t1}`. Deps: whisper-rs, model file. Fixture-tested (WAV → expected text, fuzzy).
- **detect** — `TranscriptChunk → Vec<VerseCandidate>` via the `Detector` trait. Deps: alias_engine, embedding matrix, MiniLM. Fully unit-testable from text.
- **resolver** — stateful policy: `Vec<VerseCandidate> → StagingQueue + Option<ProjectionCommand>`. Owns Confidence combination, thresholds, debounce, accept mode, correction capture. No external deps. Table-testable with candidate streams.
- **projection** — renders a `ProjectionState`. Deps: Tauri window on 2nd display.
- **alias_engine** — merges four static sources + one learned source into one lookup; exposes `resolve(token) → Option<BookOsis + weight>`. Backed by the `aliases` table. Unit-testable.
- **library** — repository over SQLite (findVerse, searchSongs, loadEmbeddings, appendSessionEvent, history). Deps: rusqlite. Temp-DB tested.
- **importer** — `translation file → validated verses → on-device embeddings → persisted index`. Deps: library, MiniLM. Sample-file tested.

### 5.3 Data model (SQLite sketch)

```
translations(id, code, name, license, is_public_domain, is_bundled)
books(id, translation_id, number, name, osis, testament)
verses(id, translation_id, book_id, chapter, verse, text)
verse_embeddings(verse_id, translation_id, vector BLOB)      -- 384×f32
verses_fts(text, verse_id)                                   -- FTS5 (rerank + lexical)
songs(id, title, author, ccli, lyrics)
song_slides(id, song_id, order_index, text)
aliases(alias, book_osis, weight, source)                    -- source ∈ {canonical,regional,phonetic,stt_error,church_custom}
settings(key, value)                                         -- config overrides
session_events(id, session_id, ts, kind, payload_json)       -- full replayable trace
```

`session_events` records the entire chain — `SpeechSegment → TranscriptChunk → VerseCandidates → OperatorAction → ProjectionChanged` — so any service can be **replayed through the pipeline** as a test/diagnostic fixture. This is the primary regression harness for detection tuning. At startup, `verse_embeddings` for active translations load once into an in-memory `{translation_id → (Vec<VerseId>, f32 matrix)}` for brute-force cosine.

### 5.4 Events & transport (modulated from review #1)

Events are defined explicitly in `core/events.rs`. Transport in Phase 1–2 is `tokio::mpsc` channels along the linear path, which gives natural backpressure. A fan-out publish/subscribe bus is intentionally **not** built yet; it earns its place only once ≥3 independent observers (diagnostics, replay logger, plugins) need the same stream, at which point it is a transport swap behind the same event types — not a redesign.

### 5.5 Confidence model (review #10)

`detect` never emits a bare float. Each candidate carries:

```rust
struct Confidence { regex: f32, alias: f32, semantic: f32, context: f32, history: f32 }
```

The **resolver** owns the combination function (weighted, tunable via `config`) → a single comparable score used for ranking, thresholding, and the auto-advance cutoff τ. Keeping the components separate makes the system tunable and debuggable instead of magic.

### 5.6 Extensibility (modulated from review #11)

- **`Detector` trait — added now.** Four concrete impls already exist (L1–L4), so this is real polymorphism, and it lets detection layers be added/reordered/tested in isolation.
- **`ProjectionOutput`, `TranslationProvider` — thin module boundaries now, traits extracted when the 2nd impl lands.** Second implementations are genuinely near-term (OBS/NDI output; imported translations vs bundled), so the boundaries stay clean and side-effect-isolated.
- **`SongProvider` / full plugin SDK — deferred.** One implementation today; declaring the interface now would encode current assumptions as future constraints. Extract when a real second consumer exists.

### 5.7 Church Learning Mode (seam promoted; feature phased — review #9)

The *seam* is core from day one: the resolver captures every operator correction as a `session_event`, and `alias_engine` exposes a `church_custom` source that these corrections feed (e.g. `Jaira→Jireh`, `Hosier→Hosea`). The learning *loop* (auto-applying learned corrections) activates alongside detection in Phase 2–3. Promoting the seam, not the timeline, is what avoids ML in Phase 1 while preserving the differentiator.

## 6. Detection Pipeline (the differentiator)

- **L1 Regex** — `John 3:16`, `Romans 8`, `1 Cor 13`, `First Corinthians 13`, `Genesis chapter 2`. ~70% of explicit references.
- **L2 Alias + fuzzy** — `alias_engine` (canonical + regional + phonetic + STT-error + church-custom) with `strsim` recovery; Bible-aware post-processor corrects misheard book names before parsing (`roots`→Ruth).
- **L3 NL cues** — trigger phrases ("turn with me to", "open your Bible to", "reading from", "beginning at verse") that mark an incoming reference and bias parsing.
- **L4 Semantic** — embed the transcript window with MiniLM → brute-force cosine → **top-20 → lexical rerank (FTS5) → top-3**. The rerank costs almost nothing over 20 candidates and fixes the main semantic failure mode: confusing near-identical verses. Catches *"The Lord is my shepherd"* → Psalm 23:1 with no explicit reference.

`detect` returns candidates with `layer` and structured `Confidence`; the **resolver** decides what (if anything) reaches the operator/projection.

## 7. Control Model

**Staging-first.** Candidates appear in a suggestions rail with confidence; operator accepts (Enter/click) to project. Wrong guesses never reach the wall. **Auto-advance** is a later per-confidence-threshold toggle (`config` τ) layered on the same pipeline (`resolver` auto-accepts above τ). Same detection code; auto-advance ≈ a 1-day addition once confidence numbers are trusted.

## 8. Roadmap (vertical slices)

- **Phase 1 — Shell + projection seam (no ML).** Tauri app, SQLite loaded with WEB, `events`/`config`/`pipeline`/`diagnostics` skeletons, `ProjectionState` machine, manual reference lookup (`John 3:16` → 2nd monitor), song entry + slide render. Proves DB schema, IPC, and the "put this on the wall" seam every later phase plugs into.
- **Phase 2 — Live STT + explicit detection.** whisper.cpp, mic + VAD, rolling transcript, `Detector` L1–L3, `alias_engine`, resolver staging queue + correction capture, operator-confirmed projection. Diagnostics latency counters go live.
- **Phase 3 — Semantic / paraphrase.** Load embedding matrix, L4 cosine + lexical rerank, structured Confidence combination, ranked paraphrase suggestions, optional auto-advance τ, church-learning loop active.
- **Phase 4 — Song library polish + import.** Auto slide splitting, FTS search, translation import pipeline (on-device indexing), keyboard-driven service workflow, session replay tooling built on `session_events`.
- **Phase 5 (optional).** OBS browser source (`ProjectionOutput` 2nd impl), NDI, Wi-Fi remote, plugin traits extracted. Only if zero cost to core.

## 9. Skills Required

- **Rust:** ownership, `tokio` async + channels, `serde`, `rusqlite`, error handling, FFI to whisper.cpp, trait design.
- **AI/runtime:** whisper.cpp/GGUF/quantization, ONNX Runtime (`ort`), embeddings + cosine similarity, lexical reranking.
- **Audio:** mic capture (`cpal`), VAD, streaming buffers.
- **NLP:** regex, tokenization, fuzzy matching (`strsim`), NER-lite for references.
- **Data:** SQLite, FTS5, in-memory vector scan.
- **UI:** React, Tailwind, Zustand, Tauri IPC, hotkeys, multi-window/second-screen.

## 10. Risks & Mitigations

- **STT accuracy in noisy rooms** — the one area cloud (Deepgram) leads. Mitigate with VAD, model choice, and the L2 Bible-aware corrector so book-name errors self-heal.
- **whisper.cpp real-time on old CPU** — tune chunk size + model tier; default `base.en`, allow `tiny.en`.
- **ORT model packaging on Windows** — verify `ort` static/dynamic linking early (Phase 3 spike, de-risked in Phase 1 if time allows).
- **Translation-import legality** — app never distributes licensed text; user supplies their own legal copy.
- **Over-abstraction creep** — event bus and plugin SDK are explicitly deferred (§3, §5.4, §5.6); revisit only when a second implementation exists.

## 11. Testing Strategy

- Unit: `detect` (text→candidates), `resolver` (candidate stream→decisions), `alias_engine` (token→book) are pure enough for extensive table-driven tests — the highest-value coverage.
- Integration: `library` against a temp SQLite; `importer` against a sample translation file.
- Fixture-based: `stt` against recorded WAV → expected text (fuzzy); `audio` against WAV with/without speech.
- **Replay:** whole services captured in `session_events` replay through the pipeline as regression fixtures for detection tuning.
- E2E: Phase-1 manual-lookup → projection happy path.
