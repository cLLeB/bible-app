import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { ProjectionState, VersePayload } from "./api";
import type { MediaKind } from "./lib/media";
import { usableCues } from "./lib/serviceCues";

export type Cue =
  | { id: string; type: "verse"; verse: VersePayload }
  | { id: string; type: "song"; songId: number; title: string }
  // Media cues store the library id, not the path: renaming or moving a file is
  // then a library concern, and a run order saved as a template last month
  // still points at the right item.
  | { id: string; type: "media"; mediaId: number; title: string; kind: MediaKind };

interface ServiceState {
  cues: Cue[];
  addVerse: (verse: VersePayload) => void;
  addSong: (songId: number, title: string) => void;
  addMedia: (mediaId: number, title: string, kind: MediaKind) => void;
  remove: (id: string) => void;
  move: (id: string, dir: -1 | 1) => void;
  clear: () => void;
  /** Replace the whole run order (used when loading a template). */
  setCues: (cues: Cue[]) => void;
}

function uid(): string {
  return Math.random().toString(36).slice(2, 10);
}

// Which surface currently owns keyboard slide navigation, so SongLive,
// ServicePanel and the scripture presenter don't all react to one key press.
type LiveOwner = "service" | "song" | "scripture" | null;
interface LiveState {
  owner: LiveOwner;
  setOwner: (owner: LiveOwner) => void;
}
export const useLiveStore = create<LiveState>((set) => ({
  owner: null,
  setOwner: (owner) => set({ owner }),
}));

/**
 * What is staged for the wall but not on it yet.
 *
 * Deliberately console-only and not persisted: a preview is the operator's
 * private working space, and it must never survive a restart into a service
 * where someone else assumes it is live.
 */
interface PreviewState {
  staged: ProjectionState | null;
  stage: (next: ProjectionState) => void;
  clear: () => void;
}
export const usePreviewStore = create<PreviewState>((set) => ({
  staged: null,
  stage: (staged) => set({ staged }),
  clear: () => set({ staged: null }),
}));

/**
 * Things put on the wall a moment ago, whatever kind of thing they were.
 *
 * This used to hold verses only, which made "Recent" quietly mean "recent
 * scripture": an operator who had just shown a song and a slide saw neither, and had
 * to go back to the panel they came from to reach them again. Mid-service, going
 * back to the panel is the expensive part.
 *
 * Not persisted. A list of what was on screen last Sunday is noise, and worse, an
 * invitation to re-project something from a different service.
 */
export type RecentItem =
  | { kind: "verse"; id: string; title: string; verse: VersePayload }
  | { kind: "song"; id: string; title: string; songId: number }
  | { kind: "media"; id: string; title: string; mediaId: number; mediaKind: MediaKind };

interface RecentState {
  items: RecentItem[];
  push: (item: RecentItem) => void;
}
export const useRecentStore = create<RecentState>((set) => ({
  items: [],
  push: (item) =>
    set((s) => ({
      // Most recent first, no duplicates, and short enough to stay one row.
      items: [item, ...s.items.filter((r) => r.id !== item.id)].slice(0, 8),
    })),
}));

/** Record a verse as recently shown. */
export function rememberVerse(verse: VersePayload): void {
  useRecentStore.getState().push({
    kind: "verse",
    id: `verse:${verse.bookOsis}:${verse.chapter}:${verse.verse}:${verse.translation}`,
    title: verse.reference,
    verse,
  });
}

export function rememberSong(songId: number, title: string): void {
  useRecentStore.getState().push({ kind: "song", id: `song:${songId}`, title, songId });
}

export function rememberMedia(mediaId: number, title: string, mediaKind: MediaKind): void {
  useRecentStore
    .getState()
    .push({ kind: "media", id: `media:${mediaId}`, title, mediaId, mediaKind });
}

// The scripture verse currently being presented (for the nav controller),
// plus a short list of recently-shown verses for instant re-projection.
interface ScriptureState {
  current: VersePayload | null;
  recents: VersePayload[];
  setCurrent: (v: VersePayload | null) => void;
  pushRecent: (v: VersePayload) => void;
}
export const useScriptureStore = create<ScriptureState>((set) => ({
  current: null,
  recents: [],
  setCurrent: (current) => set({ current }),
  pushRecent: (v) =>
    set((s) => ({
      recents: [v, ...s.recents.filter((r) => r.reference !== v.reference || r.translation !== v.translation)].slice(0, 8),
    })),
}));

export const useServiceStore = create<ServiceState>()(
  persist(
    (set) => ({
      cues: [],
      addVerse: (verse) =>
        set((s) => ({ cues: [...s.cues, { id: uid(), type: "verse", verse }] })),
      addSong: (songId, title) =>
        set((s) => ({ cues: [...s.cues, { id: uid(), type: "song", songId, title }] })),
      addMedia: (mediaId, title, kind) =>
        set((s) => ({ cues: [...s.cues, { id: uid(), type: "media", mediaId, title, kind }] })),
      remove: (id) => set((s) => ({ cues: s.cues.filter((c) => c.id !== id) })),
      move: (id, dir) =>
        set((s) => {
          const i = s.cues.findIndex((c) => c.id === id);
          const j = i + dir;
          if (i < 0 || j < 0 || j >= s.cues.length) return s;
          const cues = [...s.cues];
          [cues[i], cues[j]] = [cues[j], cues[i]];
          return { cues };
        }),
      clear: () => set({ cues: [] }),
      setCues: (cues) => set({ cues }),
    }),
    {
      name: "service-order",
      version: 1,
      partialize: (s) => ({ cues: s.cues }),
      // Stored run orders outlive the builds that wrote them, so the rule for
      // what survives lives in one tested place.
      migrate: (persisted) => ({
        cues: usableCues((persisted as { cues?: unknown[] } | undefined)?.cues ?? []),
      }),
    },
  ),
);

/** A reusable, named order of service (e.g. "Sunday Morning"). */
export interface ServiceTemplate {
  id: string;
  name: string;
  cues: Cue[];
}

interface TemplateState {
  templates: ServiceTemplate[];
  /** Save the given cues under a name; a same-name template is overwritten. */
  save: (name: string, cues: Cue[]) => void;
  remove: (id: string) => void;
}

export const useTemplateStore = create<TemplateState>()(
  persist(
    (set) => ({
      templates: [],
      save: (name, cues) =>
        set((s) => ({
          templates: [
            ...s.templates.filter((t) => t.name !== name),
            { id: uid(), name, cues },
          ].sort((a, b) => a.name.localeCompare(b.name)),
        })),
      remove: (id) => set((s) => ({ templates: s.templates.filter((t) => t.id !== id) })),
    }),
    { name: "service-templates", version: 1 },
  ),
);

/**
 * Actions the keyboard needs that live inside a panel.
 *
 * The run sheet is owned by ServicePanel, and a global key handler cannot reach into
 * it. Rather than lift the whole thing into the store - which would mean moving the
 * slide bookkeeping too - the panel registers what it can do, and unregisters when
 * it goes away. Nothing else may call these; they exist so a keypress and a click
 * take exactly the same path.
 */
interface RunSheetActions {
  projectIndex: ((i: number) => void) | null;
  step: ((delta: 1 | -1) => void) | null;
}
export const runSheet: RunSheetActions = { projectIndex: null, step: null };
