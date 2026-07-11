import { invoke } from "@tauri-apps/api/core";

export interface VersePayload {
  reference: string;
  book: string;
  chapter: number;
  verse: number;
  text: string;
  translation: string;
}

export const lookupReference = (query: string): Promise<VersePayload> =>
  invoke<VersePayload>("lookup_reference", { query });

export const projectVerse = (payload: VersePayload): Promise<void> =>
  invoke<void>("project_verse", { payload });

export const blankProjection = (): Promise<void> => invoke<void>("blank_projection");
