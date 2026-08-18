/**
 * Service-wide keyboard shortcuts.
 *
 * A service is run standing up, often in a dark booth, with one hand on the laptop
 * and eyes on the preacher. Every other platform in this space is driven by keyboard
 * for that reason. Until now only the arrow keys did anything, and only while the
 * scripture panel happened to own the live cursor.
 *
 * The rules that make this safe to have on all the time:
 *
 *   * never while typing — a shortcut that blanks the screen because someone typed
 *     "b" into the song editor is worse than no shortcut at all;
 *   * never with a modifier held, so the browser's own Ctrl/Cmd keys are untouched;
 *   * navigation still belongs to whichever panel owns the live cursor, so the arrows
 *     keep meaning what they already meant.
 */

/** A key the operator can press, and what it does. */
export interface Hotkey {
  /** `KeyboardEvent.key`, matched case-insensitively. */
  key: string;
  /** Shown in the cheat sheet. */
  label: string;
  /** Grouping in the cheat sheet. */
  group: "Projection" | "Scripture" | "Service" | "Help";
}

/** Everything bound, in the order the cheat sheet lists it. */
export const HOTKEYS: Hotkey[] = [
  { key: "Enter", label: "Put the preview live", group: "Projection" },
  { key: "b", label: "Blank the screen", group: "Projection" },
  { key: "k", label: "Blackout", group: "Projection" },
  { key: "l", label: "Logo", group: "Projection" },
  { key: "Escape", label: "Clear the alert, or the preview", group: "Projection" },
  { key: "ArrowRight", label: "Next verse", group: "Scripture" },
  { key: "ArrowLeft", label: "Previous verse", group: "Scripture" },
  { key: "ArrowDown", label: "Next chapter", group: "Scripture" },
  { key: "ArrowUp", label: "Previous chapter", group: "Scripture" },
  { key: "/", label: "Jump to the search box", group: "Scripture" },
  { key: "n", label: "Next item on the run sheet", group: "Service" },
  { key: "p", label: "Previous item", group: "Service" },
  { key: "1", label: "Item 1 to 9 on the run sheet", group: "Service" },
  { key: "s", label: "Stage display", group: "Service" },
  { key: "?", label: "Show this list", group: "Help" },
];

/** The run-sheet number keys, which the sheet lists as one row. */
export function runSheetIndex(key: string): number | null {
  if (key.length !== 1 || key < "1" || key > "9") return null;
  return Number(key) - 1;
}

/**
 * Is the operator typing into something?
 *
 * Checked before every shortcut. `isContentEditable` matters as much as the tag
 * names: a rich-text field is not an `<input>`, and someone editing lyrics in one
 * would otherwise blank the congregation screen by typing the letter b.
 */
export function isTypingTarget(el: EventTarget | null): boolean {
  const node = el as HTMLElement | null;
  if (!node) return false;
  const tag = node.tagName;
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    node.isContentEditable === true
  );
}

/**
 * Should this event be treated as a shortcut at all?
 *
 * Modifier combinations are left alone on purpose. Ctrl+L is the browser's address
 * bar and Cmd+K is a search box in half the software anyone has used; quietly
 * stealing them to blank a projector would be its own kind of bug.
 */
export function isShortcut(e: KeyboardEvent): boolean {
  if (e.ctrlKey || e.metaKey || e.altKey) return false;
  if (isTypingTarget(e.target)) return false;
  return true;
}

/** The action bound to a key, or undefined. Case-insensitive for letters. */
export function lookup(key: string): Hotkey | undefined {
  const k = key.length === 1 ? key.toLowerCase() : key;
  return HOTKEYS.find((h) => (h.key.length === 1 ? h.key.toLowerCase() : h.key) === k);
}

/** The cheat sheet's contents, grouped in the order the groups first appear. */
export function grouped(): [string, Hotkey[]][] {
  const out: [string, Hotkey[]][] = [];
  for (const h of HOTKEYS) {
    const found = out.find(([g]) => g === h.group);
    if (found) found[1].push(h);
    else out.push([h.group, [h]]);
  }
  return out;
}
