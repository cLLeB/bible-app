import { invoke } from "@tauri-apps/api/core";

export interface VersePayload {
  reference: string;
  book: string;
  bookOsis: string;
  chapter: number;
  verse: number;
  text: string;
  translation: string;
}

export type NavDir = "next-verse" | "prev-verse" | "next-chapter" | "prev-chapter";

export const navigate = (dir: NavDir): Promise<VersePayload | null> =>
  invoke<VersePayload | null>("navigate", { dir });

export const presentCoords = (
  bookOsis: string,
  chapter: number,
  verse: number,
): Promise<VersePayload> =>
  invoke<VersePayload>("present_coords", { bookOsis, chapter, verse });

export type ProjectionState =
  | { kind: "blank" }
  | { kind: "blackout" }
  | { kind: "logo" }
  | { kind: "verse"; text: string; caption: string }
  | { kind: "song"; text: string; caption: string }
  | { kind: "message"; text: string }
  | { kind: "countdown"; targetMs: number; label: string };

export interface ProjectionSettings {
  fontScale: number;
  theme: "dark" | "light" | "sepia";
}

export const setProjection = (next: ProjectionState): Promise<void> =>
  invoke<void>("set_projection", { next });

export const getProjectionSettings = (): Promise<ProjectionSettings> =>
  invoke<ProjectionSettings>("get_projection_settings");

export const setProjectionSettings = (settings: ProjectionSettings): Promise<void> =>
  invoke<void>("set_projection_settings", { settings });

export const showStage = (): Promise<void> => invoke<void>("show_stage");

export const startRemote = (): Promise<string> => invoke<string>("start_remote");

export interface SongSummary {
  id: number;
  title: string;
  author: string | null;
  builtIn: boolean;
}

export interface Slide {
  orderIndex: number;
  text: string;
}

export const lookupReference = (query: string): Promise<VersePayload> =>
  invoke<VersePayload>("lookup_reference", { query });

export const searchScripture = (query: string): Promise<VersePayload[]> =>
  invoke<VersePayload[]>("search_scripture", { query });

export const relatedVerses = (
  bookOsis: string,
  chapter: number,
  verse: number,
): Promise<VersePayload[]> =>
  invoke<VersePayload[]>("related_verses", { bookOsis, chapter, verse });

export const chunkPassage = (text: string, maxChars: number): Promise<string[]> =>
  invoke<string[]>("chunk_passage", { text, maxChars });

export const recordChoice = (
  transcript: string,
  bookOsis: string,
  chapter: number,
  verse: number,
): Promise<void> =>
  invoke<void>("record_choice", { transcript, bookOsis, chapter, verse });

export interface TranslationInfo {
  code: string;
  name: string;
}

export const listTranslations = (): Promise<TranslationInfo[]> =>
  invoke<TranslationInfo[]>("list_translations");

export const getTranslation = (): Promise<string> => invoke<string>("get_translation");

export const setTranslation = (code: string): Promise<void> =>
  invoke<void>("set_translation", { code });

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

export const exportSongs = (): Promise<string> => invoke<string>("export_songs");
export const importSongs = (json: string): Promise<number> =>
  invoke<number>("import_songs", { json });

export interface Candidate {
  verse: VersePayload;
  confidence: number;
  source: string; // "explicit" | "fuzzy" | "context"
}

export type SttModel = "tiny" | "base" | "small" | "medium";

export const startListening = (model: SttModel): Promise<void> =>
  invoke<void>("start_listening", { model });
export const stopListening = (): Promise<void> => invoke<void>("stop_listening");
