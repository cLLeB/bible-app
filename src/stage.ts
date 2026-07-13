// Helpers that feed the stage/confidence monitor. Each live surface (scripture,
// songs, service order) calls these so the stage always shows the current line
// plus a preview of what's next. Failures are swallowed — the stage monitor is
// an aid, never allowed to break the operator's main flow.
import { peekNext, setStage, type StageSlot, type VersePayload } from "./api";

export function verseSlot(v: VersePayload): StageSlot {
  return { text: v.text, caption: `${v.reference} · ${v.translation}` };
}

export function slideSlot(text: string, title: string): StageSlot {
  return { text, caption: title };
}

/// Push the live scripture verse and, from the backend cursor, its next verse.
export async function pushScriptureStage(current: VersePayload): Promise<void> {
  try {
    const nextVerse = await peekNext().catch(() => null);
    await setStage(verseSlot(current), nextVerse ? verseSlot(nextVerse) : null);
  } catch {
    /* stage is non-critical */
  }
}

/// Clear the stage's live line (e.g. when the operator blanks the screen).
export async function clearStage(): Promise<void> {
  try {
    await setStage(null, null);
  } catch {
    /* stage is non-critical */
  }
}
