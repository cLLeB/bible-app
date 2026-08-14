import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { ProjectionState, VersePayload } from "./api";
import { usableCues } from "./lib/serviceCues";

export type Cue =
  | { id: string; type: "verse"; verse: VersePayload }
  | { id: string; type: "song"; songId: number; title: string }
  // Media cues store the library id, not the path: renaming or moving a file is
  // then a library concern, and a run order saved as a template last month
  // still points at the right item.
  | { id: string; type: "media"; mediaId: number; title: string; kind: "image" | "video" };

interface ServiceState {
  cues: Cue[];
  addVerse: (verse: VersePayload) => void;
  addSong: (songId: number, title: string) => void;
  addMedia: (mediaId: number, title: string, kind: "image" | "video") => void;
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
