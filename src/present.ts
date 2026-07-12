import { projectVerse, type VersePayload } from "./api";
import { useLiveStore, useScriptureStore } from "./services";

/**
 * Project a scripture verse AND make it the active, navigable presentation:
 * sets the scripture cursor and grabs keyboard ownership so arrow keys drive
 * verse/chapter navigation. Use this everywhere a verse is put on screen.
 */
export async function present(verse: VersePayload): Promise<void> {
  useLiveStore.getState().setOwner("scripture");
  useScriptureStore.getState().setCurrent(verse);
  useScriptureStore.getState().pushRecent(verse);
  await projectVerse(verse);
}
