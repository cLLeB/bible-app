import { describe, expect, it } from "vitest";
import { grouped, HOTKEYS, isShortcut, isTypingTarget, lookup } from "./hotkeys";

/** A KeyboardEvent-shaped object; only the fields isShortcut reads. */
function ev(over: Partial<KeyboardEvent> & { target?: unknown }): KeyboardEvent {
  return {
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    target: null,
    ...over,
  } as KeyboardEvent;
}

function el(tag: string, contentEditable = false): EventTarget {
  return { tagName: tag, isContentEditable: contentEditable } as unknown as EventTarget;
}

describe("when a keypress counts as a shortcut", () => {
  it("does nothing while the operator is typing", () => {
    // The case this exists for: typing "b" into the song editor must not blank the
    // congregation screen.
    for (const tag of ["INPUT", "TEXTAREA", "SELECT"]) {
      expect(isShortcut(ev({ target: el(tag) }))).toBe(false);
    }
    expect(isShortcut(ev({ target: el("DIV", true) }))).toBe(false);
  });

  it("leaves modifier combinations to the browser", () => {
    // Ctrl+L is the address bar; Cmd+K is a search box in half the software anyone
    // has used. Stealing those to drive a projector would be its own bug.
    expect(isShortcut(ev({ ctrlKey: true }))).toBe(false);
    expect(isShortcut(ev({ metaKey: true }))).toBe(false);
    expect(isShortcut(ev({ altKey: true }))).toBe(false);
  });

  it("fires on a bare keypress outside a field", () => {
    expect(isShortcut(ev({ target: el("DIV") }))).toBe(true);
    expect(isShortcut(ev({ target: null }))).toBe(true);
  });
});

describe("looking up what a key does", () => {
  it("matches letters whatever the caps lock is doing", () => {
    expect(lookup("b")?.label).toBe("Blank the screen");
    expect(lookup("B")?.label).toBe("Blank the screen");
  });

  it("matches the named keys exactly", () => {
    expect(lookup("ArrowRight")?.label).toBe("Next verse");
    // Left/right is a verse, up/down is a chapter: two axes, two sizes of move.
    expect(lookup("ArrowDown")?.label).toBe("Next chapter");
    expect(lookup("ArrowUp")?.label).toBe("Previous chapter");
    expect(lookup("Escape")?.label).toContain("Clear the alert");
  });

  it("returns nothing for an unbound key", () => {
    expect(lookup("q")).toBeUndefined();
    expect(lookup("F7")).toBeUndefined();
  });
});

describe("the cheat sheet", () => {
  it("lists every binding exactly once", () => {
    const listed = grouped().flatMap(([, keys]) => keys);
    expect(listed).toHaveLength(HOTKEYS.length);
  });

  it("binds no key twice", () => {
    // Two actions on one key means one of them silently never runs.
    const keys = HOTKEYS.map((h) => h.key.toLowerCase());
    expect(new Set(keys).size).toBe(keys.length);
  });
});

describe("isTypingTarget", () => {
  it("ignores ordinary elements", () => {
    expect(isTypingTarget(el("DIV"))).toBe(false);
    expect(isTypingTarget(el("BUTTON"))).toBe(false);
    expect(isTypingTarget(null)).toBe(false);
  });
});
