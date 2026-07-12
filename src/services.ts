import { create } from "zustand";
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
}

function uid(): string {
  return Math.random().toString(36).slice(2, 10);
}

// Which surface currently owns keyboard slide navigation, so SongLive and
// ServicePanel don't both react to the same arrow-key press.
type LiveOwner = "service" | "song" | null;
interface LiveState {
  owner: LiveOwner;
  setOwner: (owner: LiveOwner) => void;
}
export const useLiveStore = create<LiveState>((set) => ({
  owner: null,
  setOwner: (owner) => set({ owner }),
}));

export const useServiceStore = create<ServiceState>((set) => ({
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
}));
