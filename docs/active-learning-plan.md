# Active Learning — design & build plan

On-device, per-machine, per-profile self-improvement. Everything runs inside the installed
app (bundled whisper + local storage); **no dev machine, no codebase, no internet** required.

## Locked decisions
- **Review, not silent auto-learning.** At the end of a service, show a short review; the
  operator confirms before anything is learned. Human-confirmed labels only.
- **Baked profiles are an immutable floor.** President / Vice-President ship baked; local
  learning creates a local layer that can always be **reset to the baked version**.
- **Store 16 kHz mono (downsampled/compressed)** — what the recognizer uses — to keep audio small.
- **Rolling window: default 5 sermons/profile, min 2, learn from most-recent-N.** Oldest
  beyond the cap is deleted (audio + its heavy data), all the time.

## Trust tiers for captured live moments
- **Gold** — operator projected it AND the preacher read the verse aloud (double confirm).
- **Silver** — operator manually projected / corrected it (operator is the authority).
- **Bronze** — auto-projected and she read it (affirmation only).
- **Negative** — auto-projected, operator swapped it out → a *labelled mistake* (audio → wrong
  guess → right answer). The richest signal for misheard-name learning.
- Default: Gold/Silver drive re-tuning; Negatives feed alias learning; Bronze used with care.

## Two stores, different lifetimes
- **Rolling audio (≤5, 16k mono, compressed):** for re-tuning *acoustic* settings.
- **Permanent knowledge log (tiny text):** confirmed (spoken-phrase → reference) pairs,
  misheard names, threshold, version. Never deleted — the profile keeps improving on
  vocabulary/corrections even after old audio is cleaned up.

## Learn different facets at different cadences
- Misheard names / corrections → apply almost immediately (additive, low-risk; already guarded
  by the full-reference rule).
- Sound-desk threshold → update easily from recent audio (feeds drift).
- Recognizer settings (window/beam) → only re-tune with **enough** new audio (the "not one
  sermon overrides ours" rule). These are the ones that can go wrong.

## Never overwrite blindly
On re-learn: keep the previous version, **measure new vs recent services** ("new 9/12 vs old
7/12 — keep it?"), operator accepts or rolls back. Baked floor always restorable.

## Policy
- Baked profile (President/VP): local re-tune of acoustic settings only when ≥ min sermons
  gathered (never a single sermon). Corrections/aliases may apply sooner (guarded).
- New profile: may start from a single sermon and grow toward the cap.
- A service with too few confirmed references doesn't count toward the window.

## Idle scheduler (safety)
Learn only when: app open, not listening, not projecting, on mains power (heavy CPU), past a
cooldown. Dismissible prompt; **instant pause** the moment the operator touches the app;
checkpoint so pause/resume/crash-recovery works. Manual start/stop available.

## Consent / privacy
- **Opt-in per profile** ("record services to improve President") — recording a preacher, even
  locally, is a knowing choice; message "nothing leaves this machine."
- **Installer EULA / privacy acceptance** required before install completes (see below).

## Phases
1. **Phase 1** — "Record this service" → rolling 5 (16k mono) per profile + capture
   Gold/Silver/Negative moments → end-of-service review → learn-when-idle on approved data.
   Non-destructive / versioned. Baked floor. + installer EULA.
2. **Phase 2** — permanent knowledge log + per-profile accuracy stats + data-driven auto-threshold.
3. **Phase 3** — full pause/resume/scheduler + polish.
4. **UI pass** — after the new controls exist, rethink layout/arrangement/sizing so the console
   doesn't get crowded (grouping, progressive disclosure, sensible defaults hidden).
