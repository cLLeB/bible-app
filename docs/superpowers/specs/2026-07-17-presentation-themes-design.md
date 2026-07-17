# Presentation Themes — design (Slice 1 of the presentation suite)

**Date:** 2026-07-17
**Status:** Implemented
**Context:** First slice of the Phase-4 "presentation suite" (`docs/ROADMAP.md`),
building the platform layer on top of the existing app. Derived from what the
cloned reference platforms (`docs/REFERENCE-ANALYSIS.md`) all share: a themeable
projection look is the foundation every other visual feature renders through.

## Problem

The projection screen had three hardcoded looks (`dark`/`light`/`sepia` colour
triplets) and a global font scale. No custom themes, no gradients, no font or
text-style control — so verses and songs always landed as plain text on a plain
background. Every clone platform (ProPresenter/FreeShow/EasyWorship) is built on a
real, editable theme model.

## What this slice adds

A **theme** = a *background* (solid colour or linear gradient) + a *text style*
(font family, colour, caption colour, alignment, weight, legibility shadow,
uppercase). Themes are pure data; they serialise straight to the projection window
and persist across restarts.

- 5 built-in themes (`dark`/`light`/`sepia` preserve the old look, `spotlight` and
  `ocean` show off gradients + shadow). Built-ins can't be deleted, only duplicated.
- Operator can create/duplicate/edit/delete **custom themes** with a live preview,
  pick the active one, and adjust the global font scale.
- Appearance **persists** (SQLite `settings`: `projection:active_theme`,
  `projection:font_scale`, `theme:<id>` per custom theme). Previously settings
  reset to dark/1.0 on every launch.

**Explicitly out of scope (next slice — media):** image/video backgrounds. The
`Background` type is forward-compatible (a `kind` discriminant) so that becomes a
render-only addition, not a data-model change.

## Architecture (fits the frozen `ProjectionState` seam)

- `src-tauri/src/themes.rs` — theme model, built-ins, and `settings`-backed
  persistence (pure/DB logic, unit-tested).
- `events.rs::ProjectionSettings` now carries the fully-resolved active `Theme`
  (not a string), so the projection window renders straight from the payload.
- Commands: `list_themes`, `set_active_theme`, `set_font_scale`, `save_theme`,
  `delete_theme` (replacing the old `set_projection_settings`). Every appearance
  change flows through one `apply_settings` seam that stores + emits `set-settings`.
- Startup loads persisted settings into `AppState`.
- Frontend: `src/lib/theme.ts` — pure theme→CSS + auto-fit sizing (unit-tested,
  shared by the live projection and the editor preview so they always match);
  `ThemesPanel.tsx` — library + editor + preview; `DisplayPanel` keeps the live
  font slider; `ProjectionView` renders through the resolved theme.

## Testing

- Rust: `themes` unit tests (built-in stability, camelCase round-trip, custom
  save/list/delete + fallback). Full suite: 125 pass.
- Frontend: `theme.test.ts` (backgroundCss, auto-fit, body/caption style) — 8 pass,
  via newly-added Vitest. `npm run build` type-checks clean.
- Not yet exercised in a live GUI run — that's the manual verification step.
