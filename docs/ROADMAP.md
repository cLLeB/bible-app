# Feature Roadmap — parity with (and beyond) the leading platforms

**Researched 2026-07-11** against ProPresenter, EasyWorship, OpenLP, FreeShow, and the AI-detection tools (EasyVerse / Rhema). This catalogs the *basic + intermediate* features those platforms ship, tiered and slotted into phases. The frozen architecture (`specs/2026-07-11-offline-bible-app-design.md`) already supports all of it via the `ProjectionState` seam — these are features, not architecture changes.

**Our north star:** everything below, but **fully offline** and **lighter on old hardware** than any of them, with the AI verse/paraphrase detection (Phases 2–3) as the differentiator none of the offline tools have.

**Licensing reality (unchanged):** copyrighted song lyrics (CCLI SongSelect, the "500k song" libraries) and copyrighted Bible translations **cannot be bundled** — we ship public-domain content + a legal user-import path. This is a feature (offline + your own licensed content), not a limitation.

Tiers: 🟢 Basic (table stakes) · 🟡 Intermediate · 🔴 Advanced. Phase = when we build it.

---

## Scripture
| Feature | Tier | Phase | Seen in |
|---|---|---|---|
| Reference lookup → project | 🟢 | ✅ 1A | all |
| Chapter-default-to-v1, error handling | 🟢 | ✅ 1A | all |
| **Verse ranges** (`John 3:16-18`) | 🟢 | ✅ 1C | all |
| **Keyword/phrase search** ("rejoice" → verses) | 🟡 | ✅ 1C/4 | EasyWorship, OpenLP |
| **Multiple translations + quick switch** | 🟡 | ✅ 4 | ProPresenter (130+), EasyWorship (90+) |
| **Parallel/side-by-side translations** | 🔴 | ✅ 4 | ProPresenter |
| Continuous-scroll reading (no dropdowns) | 🟡 | 4 | EasyWorship |
| **Auto verse detection from speech** (our differentiator) | 🔴 | 2 | EasyVerse, Rhema |
| **Paraphrase/semantic detection** (our differentiator) | 🔴 | 3 | (none offline) |

## Songs
| Feature | Tier | Phase | Seen in |
|---|---|---|---|
| Add song + auto slide-split | 🟢 | ✅ 1B | all |
| **Edit / update / delete song** | 🟢 | ✅ 1C | all |
| **Smart lyric paste** (auto-split, no manual blank lines, live preview) | 🟢 | ✅ 1C | all |
| **Verse/section tagging** (Verse/Chorus/Bridge) + reorder | 🟡 | 4 | OpenLP, ProPresenter |
| Author/metadata management | 🟡 | 4 | OpenLP |
| Song search | 🟢 | ✅ 1C/4 | all |
| **Bundled public-domain hymn starter set** | 🟡 | 4 | (EasyWorship ships libraries) |
| **User import** — file import ✅; CCLI SongSelect still to do | 🟡 | 5 | EasyWorship, ProPresenter |
| Copyright/CCLI reporting fields | 🟡 | 4 | EasyWorship, ProPresenter |
| Chord charts / backing tracks | 🔴 | 5+ | OpenLP, ProPresenter |

## Live operation / control
| Feature | Tier | Phase | Seen in |
|---|---|---|---|
| Project / Blank | 🟢 | ✅ 1A/1B | all |
| **Keyboard-driven live mode** (arrow keys advance + auto-project) | 🟢 | ✅ 1C | all |
| **Next/prev buttons + current-slide highlight** | 🟢 | ✅ 1C | all |
| **Blank / Logo / Blackout states** wired to UI | 🟢 | ✅ 1C | all |
| Operator **preview (next) vs live (current)** panes | 🟡 | ✅ 4 | ProPresenter, EasyWorship |
| **Remote control** (web app from phone/tablet on LAN) | 🔴 | ✅ 5 | ProPresenter, OpenLP |
| Live-edit on the fly without interrupting output | 🟡 | 4 | ProPresenter |

## Service planning
| Feature | Tier | Phase | Seen in |
|---|---|---|---|
| **Service order / schedule** (queue songs+scripture+media+announcements) | 🟡 | ✅ 4 | all |
| Drag-drop items into the order | 🟡 | 4 | EasyWorship, OpenLP |
| **Reusable service templates** | 🟡 | ✅ 4 | ProPresenter, EasyWorship |
| "What's next" running order visible to operator | 🟡 | ✅ 4 | all |

## Displays & outputs
| Feature | Tier | Phase | Seen in |
|---|---|---|---|
| Second-monitor fullscreen projection | 🟢 | ✅ 1A | all |
| **Stage / confidence monitor** (lyrics + notes + clock + timer, independent) | 🟡 | ✅ 4 | all |
| Multiple **independent** outputs (not just mirror) | 🔴 | 4/5 | ProPresenter (8), EasyWorship |
| **NDI output** to OBS / vMix | 🔴 | 5 | ProPresenter, EasyVerse |
| Per-output resolution/layout | 🔴 | 5 | ProPresenter |

## Media
| Feature | Tier | Phase | Seen in |
|---|---|---|---|
| **Image backgrounds** behind verse/lyrics | 🟡 | ✅ 4 | all |
| **Video / motion backgrounds** (loops) | 🟡 | ✅ 4 | all |
| Audio playback | 🔴 | 5 | OpenLP, ProPresenter |
| Image slideshows | 🟡 | ✅ 4 | OpenLP |
| **PowerPoint / Impress import** | 🔴 | ✅ 5 | OpenLP, EasyWorship |

## Announcements & alerts
| Feature | Tier | Phase | Seen in |
|---|---|---|---|
| **On-screen message / alert** (lower third, editable live) | 🟡 | ✅ 4 | ProPresenter, EasyWorship |
| Scrolling announcement layer (lobby loop) | 🔴 | 5 | ProPresenter |

## Timers
| Feature | Tier | Phase | Seen in |
|---|---|---|---|
| **Countdown timer** (welcome/worship/sermon/offering) | 🟡 | ✅ 4 | all |
| Linked/auto-chained timers | 🔴 | 5 | ProPresenter |
| Clock on stage display | 🟡 | ✅ 4 | all |

## Design & theming
| Feature | Tier | Phase | Seen in |
|---|---|---|---|
| **Font size / scale / alignment controls** | 🟢 | ✅ 1C/4 | all |
| **Themes** (background + text style) + one-click apply | 🟡 | ✅ 4 | ProPresenter, EasyWorship |
| Slide editor (layout, layers) | 🔴 | 4/5 | ProPresenter |
| Reusable text/verse/song templates | 🟡 | 4 | ProPresenter |

## Platform / UX polish
| Feature | Tier | Phase | Seen in |
|---|---|---|---|
| Dark mode | 🟢 | ✅ 1C/4 | all |
| Global hotkeys + cheat sheet | 🟡 | 1C/4 | all |
| Autosave / persistence | 🟢 | ✅ 1B | all |
| Global search (songs + scripture) | 🟡 | 4 | EasyWorship |
| Cross-platform (Win/Mac/Linux) | 🟡 | later | OpenLP, ProPresenter |

---

## Revised phase plan (features layered on the frozen architecture)

- **Phase 1C — Usability pass (NOW):** song edit/delete, smart lyric paste (auto-split + live preview), keyboard live mode (arrow-key slide nav + auto-project), next/prev + current-slide highlight, Blank/Logo/Blackout wired, verse ranges, basic font-scale control. → *turns the app from demo into operable, no AI needed.*
- **Phase 2 — Offline STT + explicit detection** (+ config/pipeline/diagnostics skeletons). *The differentiator begins.*
- **Phase 3 — Semantic / paraphrase detection + church-learning.** *The differentiator none of the offline tools have.*
- **Phase 4 — Service & presentation suite:** service order + templates, media (images/video/motion backgrounds), themes + font controls, countdown timers, on-screen alerts, stage/confidence monitor, multi-translation + search. *Parity with the mainstream tools.*
- **Phase 5 — Broadcast & integration:** NDI/OBS output, LAN remote control, CCLI/SongSelect + PowerPoint import, multiple independent outputs, plugin SDK. *Pro-tier reach.*

Anything new still lands in `BACKLOG.md` first; only proven architectural gaps amend the frozen spec.
