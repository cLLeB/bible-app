import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { VersePayload } from "./api";

export type Cue =
  | { id: string; type: "verse"; verse: VersePayload }
  | { id: string; type: "song"; songId: number; title: string };

interface ServiceState {
  cues: Cue[];
  addVerse: (verse: VersePayload) => void;
  addSong: (songId: number, title: string) => void;
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
      // Older builds stored verse cues without `bookOsis`, which can't project.
      // Drop any incomplete cue so the run sheet only holds usable items.
      migrate: (persisted) => {
        const raw = (persisted as { cues?: Cue[] } | undefined)?.cues ?? [];
        const cues = raw.filter((c) =>
          c && c.type === "song"
            ? c.songId != null
            : Boolean(c && c.type === "verse" && c.verse?.bookOsis != null),
        );
        return { cues };
      },
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
