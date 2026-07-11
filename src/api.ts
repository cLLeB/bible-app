import { invoke } from "@tauri-apps/api/core";

export interface VersePayload {
  reference: string;
  book: string;
  chapter: number;
  verse: number;
  text: string;
  translation: string;
}

export type ProjectionState =
  | { kind: "blank" }
  | { kind: "verse"; text: string; caption: string }
  | { kind: "song"; text: string; caption: string };

export interface SongSummary {
  id: number;
  title: string;
  author: string | null;
}

export interface Slide {
  orderIndex: number;
  text: string;
}

export const lookupReference = (query: string): Promise<VersePayload> =>
  invoke<VersePayload>("lookup_reference", { query });

export const projectVerse = (payload: VersePayload): Promise<void> =>
  invoke<void>("project_verse", { payload });

export const blankProjection = (): Promise<void> => invoke<void>("blank_projection");

export const getProjection = (): Promise<ProjectionState> =>
  invoke<ProjectionState>("get_projection");

export const addSong = (
  title: string,
  author: string | null,
  lyrics: string,
): Promise<number> => invoke<number>("add_song", { title, author, lyrics });

export const listSongs = (): Promise<SongSummary[]> => invoke<SongSummary[]>("list_songs");

export const getSongSlides = (songId: number): Promise<Slide[]> =>
  invoke<Slide[]>("get_song_slides", { songId });

export const projectSlide = (songId: number, index: number): Promise<void> =>
  invoke<void>("project_slide", { songId, index });

export interface SongDetail {
  id: number;
  title: string;
  author: string | null;
  lyrics: string;
}

export const getSong = (songId: number): Promise<SongDetail | null> =>
  invoke<SongDetail | null>("get_song", { songId });

export const updateSong = (
  songId: number,
  title: string,
  author: string | null,
  lyrics: string,
): Promise<void> => invoke<void>("update_song", { songId, title, author, lyrics });

export const deleteSong = (songId: number): Promise<void> =>
  invoke<void>("delete_song", { songId });

export const startListening = (model: "base" | "tiny"): Promise<void> =>
  invoke<void>("start_listening", { model });
export const stopListening = (): Promise<void> => invoke<void>("stop_listening");
