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

## Candidate ideas (unprioritized)
_(empty — add as they come up)_
