import { describe, expect, it } from "vitest";
import {
  capPerKind,
  rank,
  titleMatches,
  worthSearching,
  type Hit,
} from "./search";

const hit = (kind: Hit["kind"], id: string): Hit => ({ kind, id, title: id });

describe("matching a title", () => {
  it("matches every word, in any order", () => {
    // An operator types what they remember, not what is on the label.
    expect(titleMatches("Amazing Grace", "grace amazing")).toBe(true);
    expect(titleMatches("Amazing Grace", "amaz")).toBe(true);
  });

  it("needs all the words, not just one", () => {
    expect(titleMatches("Amazing Grace", "amazing love")).toBe(false);
  });

  it("ignores case", () => {
    expect(titleMatches("How Great Thou Art", "GREAT thou")).toBe(true);
  });

  it("treats an empty query as no match", () => {
    // Otherwise the whole library appears the moment the box is focused.
    expect(titleMatches("Anything", "")).toBe(false);
    expect(titleMatches("Anything", "   ")).toBe(false);
  });
});

describe("ordering the results", () => {
  // There is no "is this a reference" test any more, and deliberately so. The
  // question is answered by the backend's own parser, which knows the books, the
  // abbreviations and the spoken forms. A regex here recognised "John 3:3" and not
  // "John chapter 3 verse 3", so Enter fell through to the text search and opened
  // Luke 17:36 - a verse that merely contained those words.

  it("puts a typed reference first", () => {
    // Someone who types "John 3:16" wants that verse, not a discussion of it.
    const out = rank([hit("media", "m"), hit("verse", "v"), hit("reference", "r")]);
    expect(out[0].kind).toBe("reference");
  });

  it("puts songs above verse text and media last", () => {
    const out = rank([hit("media", "m"), hit("verse", "v"), hit("song", "s")]);
    expect(out.map((h) => h.kind)).toEqual(["song", "verse", "media"]);
  });

  it("does not mutate what it was given", () => {
    const input = [hit("media", "m"), hit("reference", "r")];
    rank(input);
    expect(input[0].kind).toBe("media");
  });
});

describe("capping each kind", () => {
  it("stops one kind burying the rest", () => {
    const many = [
      ...Array.from({ length: 10 }, (_, i) => hit("verse", `v${i}`)),
      hit("song", "s"),
    ];
    const out = capPerKind(many, 3);
    expect(out.filter((h) => h.kind === "verse")).toHaveLength(3);
    expect(out.filter((h) => h.kind === "song")).toHaveLength(1);
  });

  it("keeps the first ones, which are the best ones", () => {
    const out = capPerKind([hit("verse", "a"), hit("verse", "b"), hit("verse", "c")], 2);
    expect(out.map((h) => h.id)).toEqual(["a", "b"]);
  });
});

describe("when to search at all", () => {
  it("waits for more than one letter", () => {
    expect(worthSearching("a")).toBe(false);
    expect(worthSearching(" ")).toBe(false);
    expect(worthSearching("am")).toBe(true);
  });
});
