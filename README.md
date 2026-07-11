# Bible App (offline)

Phase 1A: manual reference lookup → second-monitor projection, WEB in SQLite. No cloud, no AI yet — this is the projection seam every later phase plugs into.

## Dev setup
1. Install Rust, Node 18+, and (Windows) MSVC Build Tools + WebView2.
2. `npm install`
3. Provide Bible data: download a public-domain WEB JSON, then
   `python scripts/normalize_web.py <source_web.json>` (writes `data/web.canonical.json`).
   Until you do, a 3-verse placeholder (`data/fixtures/web.sample.json` copied to
   `data/web.canonical.json`) lets you smoke-test `John 3:16`, `Psalm 23`, `Romans 8:28`.
4. `npm run tauri dev`

Type a reference (e.g. `John 3:16`) → **Look up** → **Project**. With a second monitor
connected, the verse fills it on black; **Blank** clears it.

## Tests
- Rust: `cd src-tauri && cargo test` (13 tests)
- Frontend build/type-check: `npm run build`

See `docs/superpowers/specs/2026-07-11-offline-bible-app-design.md` for the frozen architecture
and `docs/superpowers/plans/2026-07-11-phase1-core-seam.md` for this phase's plan.
