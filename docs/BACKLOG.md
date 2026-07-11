# Backlog / Future Enhancements

The architecture in `superpowers/specs/2026-07-11-offline-bible-app-design.md` is **frozen**. Ideas that are not part of that frozen design land here first. Nothing here enters the design doc unless it fixes a *proven* architectural gap found while building.

## Rules
- New feature idea → add it here, do not touch the frozen spec.
- Promote to the spec only with evidence of a real architectural gap (not speculation).
- Keep entries short: what, why, rough phase.

## Deferred by design (already decided — see spec §3, §5.4, §5.6)
- General publish/subscribe **event bus** — only when ≥3 independent observers need the stream.
- Full **plugin SDK** / `SongProvider` trait — only when a real second implementation exists.
- **OBS browser source, NDI, Wi-Fi remote** — Phase 5, only if zero cost to core.
- Stage display, QR remotes, themes/timelines, playlists, cloud sync.

## Song usability (raised 2026-07-11 — needed for production parity)
- **Edit / update / delete songs** — currently add-only. Basic CRUD for the song manager. _(near-term; small)_
- **Smart lyric paste** — auto-split pasted lyrics into slides without manually inserting blank lines: detect stanza breaks, "Verse/Chorus/Bridge" markers, or split every N lines; live slide preview while pasting so the operator sees/adjusts the split. _(near-term; the manual blank-line step is friction)_
- **Bundled public-domain hymn library** — ship a starter set of PD hymns (Amazing Grace, etc.). NOTE: modern/CCLI worship songs are copyrighted → user-import only, same licensing model as licensed Bible translations. _(Phase 4-ish)_
- **Song import** — paste/import from a file; later CCLI/SongSelect-style import for churches with a license.

## Live presentation / operator control (raised 2026-07-11 — core to being usable like RhemaCast/EasyVerse)
- **Keyboard-driven live mode** — load a song/verse into a "live" view; **arrow keys advance/retreat slides** and auto-project the current one; space/Esc to blank. Operator window holds focus + current index; projection stays a dumb display. _(spec Phase 4 "keyboard-driven service workflow"; strongly wanted — consider pulling earlier)_
- **Next/prev slide buttons + current-slide highlight** in the operator console.
- **Presentation polish** — font size/scale control, themes/background, verse+song visual templates. _(Phase 5)_

## Parity gaps vs. the 3 production platforms (tracking, not yet scheduled)
- Playlists / service order (queue of verses + songs to step through).
- Logo/blackout/split-screen projection states (enum already exists in spec §5.1; wire the UI).
- Multi-translation display + quick translation switch.
