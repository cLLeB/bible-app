# Reference Analysis — what to take, what to leave

_Written 2026-07-17. Grounds the the app platform direction in four open-source church
presentation apps cloned locally for study at
`../_refs-church-presentation/` (sibling to this repo, never part of our build)._

The clones:

| App | Stack | Size | Why it's here |
|-----|-------|------|---------------|
| **FreeShow** | Svelte + Electron + TS | 21 MB | Closest to us architecturally (web-tech UI, JS/TS). Modern feature set. The blueprint for our platform layer. |
| **OpenLP** | Python + Qt | 78 MB | The most *feature-complete and battle-tested*. Best taxonomy of "what a church app must do." |
| **Quelea** | Java + JavaFX | 36 MB | Best-in-class live video backgrounds + scheduling UX. |
| **SoftProjector** | C++ + Qt/QML | 8 MB | Minimalist reference: multi-screen + RTL/multilingual rendering on modest hardware. |

---

## 0. The one rule that governs all of this: **licensing**

**Every one of these four is GPL** (FreeShow GPL-3.0, OpenLP GPL-2.0+, Quelea GPL-3.0,
SoftProjector GPL-3.0).

That means: **copying their source code into the app would force the app to become GPL too** —
we'd lose the freedom to license or sell it however we want later. This is the same
licensing boundary we already respect on the Bible-translation side.

So the rule is simple and non-negotiable:

> **Study them. Reimplement the good ideas ourselves. Never paste their code.**

Ideas, feature lists, data-format shapes, and UX patterns are **not** copyrightable —
we can freely learn "a Show is an ordered list of slides with layers" or "songs need
CCLI fields." Specific *source code* is copyrighted — we don't lift it. This keeps
the app's future 100% ours. The local clones are a **reading library**, not a parts bin.

---

## 1. FreeShow — the architectural blueprint

FreeShow is the one to learn structure from, because it solved the same problem we're
about to (a web-tech UI driving real presentation output) and its layout maps almost
1:1 onto our React + Tauri world. Its `src/` splits into `frontend/` (the editor UI),
`electron/` (native/OS layer — our Rust/Tauri side), `server/` (the network layer), and
`common/` (shared model incl. `scripture/`).

**Take:**

- **The "Show / Slide" data model.** This is the single biggest thing we're missing. A
  *Show* = an ordered collection of *slides*; each slide has *layers* (text, media,
  overlays), belongs to *groups* (verse/chorus/etc.), and renders through a *template*.
  Everything — songs, scripture, announcements — becomes "a Show." Our current app
  presents scripture and songs as bespoke views; the platform needs this one unifying
  abstraction underneath them. **This is Phase 1.**
- **A real output/projection engine** (`electron/output`, `frontend/show`): multiple
  independent outputs, a stage/confidence display, slide transitions, per-output styling.
  We already have `projection` and `stage` windows in `tauri.conf.json` — we grow them
  into this.
- **The network layer** (`server/` — express + socket.io): a phone/tablet becomes a
  **remote control**, a **stage-display monitor**, or an **output viewer** over LAN. This
  is a headline feature and very reachable for us (Tauri can host a local server the same
  way).
- **Pro-AV output**: **NDI** (they use the `grandiose` lib) and **Blackmagic/Decklink**
  capture/output (`electron/blackmagic`), plus **WebRTC** output streaming and
  **timecode** sync. This is what separates "a slideshow" from "a production tool."
- **Media pipeline**: image/video/audio backgrounds, and **converters** for importing
  from ProPresenter / PowerPoint / other apps — huge for adoption (people won't switch if
  they can't bring their content).
- **Template/theme system** so slide look is data, not code.

**Leave:**

- **Electron itself.** This is our biggest edge — we stay on **Tauri** (Rust core, a
  fraction of the RAM/disk, faster cold start). FreeShow ships a ~150 MB+ Electron blob;
  we don't have to.
- Their cloud-sync layer (`electron/cloud`) — defer indefinitely; offline-first is our
  identity.
- Their large monolithic frontend stores — we already use Zustand deliberately; keep our
  smaller-files discipline instead of porting their store shape.

## 2. OpenLP — the feature checklist

Don't copy a line of OpenLP, but treat its **plugin list as the definitive "done" list**
for a serious church app. Its plugins are literally: `bibles`, `songs`, `custom`,
`images`, `media`, `presentations`, `alerts`, `songusage`, `planningcenter`, `obs_studio`;
its `core/` adds `api`, `projectors`, `display`, `server`.

**Take (as features to build, our own way):**

- **Songs, done properly**: a real song library with **CCLI fields**, flexible **verse
  ordering** (V1 C V2 C B C), and **import from every format churches already have** —
  OpenLyrics, ChordPro, OpenSong, EasyWorship, SongSelect. Import breadth is what wins
  switchers.
- **`songusage` → CCLI reporting.** Churches are legally required to report song usage to
  CCLI. Almost no free tool does this well. Cheap for us to log, and a real differentiator.
- **`planningcenter`**: import a service plan straight from **Planning Center Online**.
  Many churches already build their order of service there. Big adoption lever.
- **`projectors` → PJLink**: control projectors over the network (power on/off, shutter,
  input select) from inside the app. Very "pro," rarely free.
- **`alerts`**: lower-third alerts (nursery/parent pager, announcements) over live output.
- **`presentations`**: drive an external PowerPoint/Impress/PDF and show its slides inline.
- **A documented remote API** (`core/api`, `server.py`): stable REST/WebSocket surface so
  third parties (and our own phone remote) can drive the app.
- **Bibles**: multi-version, parallel/side-by-side, verse ranges, fast search. *We already
  have a strong Bible core here — we're ahead of OpenLP on the detection side.*

**Leave:**

- The Qt/Python stack and its decades of DB-migration baggage.
- The heavyweight plugin framework machinery — we want the *features*, not a plugin OS.

## 3. Quelea — the polish on live output

**Take:**

- **Live video backgrounds with lyrics overlaid smoothly** — Quelea's signature; the bar
  for how good moving backgrounds should look.
- **Side-by-side multiple Bible translations** on one slide.
- **Schedule / order-of-service UX** that's genuinely pleasant to build live.
- **Edit-the-live-slide-in-place** without dropping the projection.
- **Notices/ticker + countdown timers** (pre-service countdown, etc.).
- Video / live-camera / DVD as a background source.

**Leave:** the JavaFX/Java stack entirely.

## 4. SoftProjector — the minimalism check

Small and focused. **Take** its lessons on clean multi-screen handling and solid
**right-to-left / multilingual** text rendering on low-end machines. Otherwise it's
mostly superseded by the three above — keep it as a "how little can this be" reference so
we don't over-build.

---

## 5. What we already have that NONE of them do — our moat

This is the part worth protecting. The four references are all "operator drives the
slides." the app's fork brings something none of them have:

- **Live speech-to-text scripture detection** (whisper.cpp): the app *listens* to the
  preacher and auto-surfaces the verse being referenced or quoted — no operator needed.
- **On-device semantic detection** (MiniLM) + **per-preacher active-learning profiles**
  that improve over time, fully offline.
- **Offline-first, Tauri-light, Rust core.**

So the whole thesis of the app in one line:

> **A FreeShow-class presentation platform, with OpenLP-class feature completeness,
> plus a live-listening auto-scripture engine nobody else has — and all of it offline
> and lightweight because it's Tauri, not Electron.**

## 6. The "not worth it long-term" list (things we deliberately refuse)

The user's instinct — "take only the good, not the things not worth it long-term" —
concretely means we say **no** to:

- **Electron-scale bloat** (Tauri instead).
- **Cloud/account lock-in** and always-online requirements.
- **Kitchen-sink feature sprawl** with no cohesion — every feature routes through the one
  Show/output model, or it doesn't ship.
- **Overgrown plugin frameworks** that make the app slow and hard to reason about.
- **Dated / skeuomorphic UI.**
- **Dependencies on copyrighted content** we can't distribute (same boundary as our Bible
  translations).
- **Legacy DB-migration debt** — we start clean.
