import { describe, expect, it } from "vitest";
import { blockAt, replaceBlock, sections, shortLabel, type SlideLike } from "./sections";

const slide = (orderIndex: number, label: string | null, text = "x"): SlideLike => ({
  orderIndex,
  label,
  text,
});

describe("which sections get a button", () => {
  it("lists them in the order they first appear", () => {
    const out = sections([
      slide(0, "Verse 1"),
      slide(1, "Chorus"),
      slide(2, "Verse 2"),
      slide(3, "Bridge"),
    ]);
    expect(out.map((s) => s.label)).toEqual(["Verse 1", "Chorus", "Verse 2", "Bridge"]);
  });

  it("gives a repeated chorus one button, pointing at its first slide", () => {
    // A chorus sung four times is one destination, not four.
    const out = sections([
      slide(0, "Verse 1"),
      slide(1, "Chorus"),
      slide(2, "Verse 2"),
      slide(3, "Chorus"),
    ]);
    expect(out).toHaveLength(3);
    expect(out.find((s) => s.label === "Chorus")?.orderIndex).toBe(1);
  });

  it("ignores case when deciding two labels are the same section", () => {
    const out = sections([slide(0, "Chorus"), slide(1, "CHORUS")]);
    expect(out).toHaveLength(1);
  });

  it("leaves unlabelled slides out", () => {
    // A button called "Slide 7" says nothing the list does not already show.
    const out = sections([slide(0, null), slide(1, "  "), slide(2, "Verse 1")]);
    expect(out.map((s) => s.label)).toEqual(["Verse 1"]);
  });

  it("copes with a song that has no labels at all", () => {
    expect(sections([slide(0, null), slide(1, null)])).toEqual([]);
  });
});

describe("shortening a label for a button", () => {
  it("abbreviates the usual sections", () => {
    expect(shortLabel("Verse 1")).toBe("V1");
    expect(shortLabel("Chorus")).toBe("C");
    expect(shortLabel("Pre-Chorus")).toBe("PC");
    expect(shortLabel("Bridge")).toBe("B");
    expect(shortLabel("Verse 12")).toBe("V12");
  });

  it("keeps something readable for anything else", () => {
    // Better a stub of the real word than an invented abbreviation.
    expect(shortLabel("Coda")).toBe("Co");
  });
});

describe("editing one slide of a song", () => {
  const lyrics = "Verse one line\nsecond line\n\nChorus here\n\nVerse two";

  it("replaces the block at that index", () => {
    expect(replaceBlock(lyrics, 1, "New chorus")).toBe(
      "Verse one line\nsecond line\n\nNew chorus\n\nVerse two",
    );
  });

  it("edits the right repeat when a block appears twice", () => {
    // The case that makes index the only safe key: replacing "the block that looks
    // like this" would edit whichever repeat came first.
    const repeated = "Chorus\n\nVerse\n\nChorus";
    expect(replaceBlock(repeated, 2, "Changed")).toBe("Chorus\n\nVerse\n\nChanged");
  });

  it("leaves the song alone for an index that is not there", () => {
    expect(replaceBlock(lyrics, 9, "nope")).toBe(lyrics);
    expect(replaceBlock(lyrics, -1, "nope")).toBe(lyrics);
  });

  it("reads back the block an editor should open on", () => {
    expect(blockAt(lyrics, 0)).toBe("Verse one line\nsecond line");
    expect(blockAt(lyrics, 1)).toBe("Chorus here");
    expect(blockAt(lyrics, 9)).toBe("");
  });

  it("round-trips: reading a block and writing it back changes nothing", () => {
    expect(replaceBlock(lyrics, 1, blockAt(lyrics, 1))).toBe(lyrics);
  });
});
