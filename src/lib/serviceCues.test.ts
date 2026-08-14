import { describe, expect, it } from "vitest";
import { usableCues } from "./serviceCues";

const verse = {
  id: "a",
  type: "verse",
  verse: { reference: "John 3:16", bookOsis: "John", chapter: 3, verse: 16 },
};
const song = { id: "b", type: "song", songId: 4, title: "Amazing Grace" };
const media = { id: "c", type: "media", mediaId: 7, title: "Bumper", kind: "video" };

describe("usableCues", () => {
  it("keeps every cue kind the runner can project", () => {
    // Media cues joined the run order after verses and songs. A stored order
    // written today must not lose them on the next release.
    expect(usableCues([verse, song, media])).toHaveLength(3);
    expect(usableCues([media])[0]).toMatchObject({ type: "media", mediaId: 7 });
  });

  it("drops cues an older build wrote that cannot project", () => {
    const legacyVerse = { id: "d", type: "verse", verse: { reference: "John 3:16" } };
    expect(usableCues([legacyVerse, verse])).toEqual([verse]);
  });

  it("drops incomplete and unrecognised shapes rather than failing mid-service", () => {
    expect(usableCues([{ id: "e", type: "song" }])).toEqual([]);
    expect(usableCues([{ id: "f", type: "media" }])).toEqual([]);
    expect(usableCues([{ id: "g", type: "countdown" }])).toEqual([]);
    expect(usableCues([null, undefined, 7, "song"])).toEqual([]);
  });

  it("survives an empty or absent stored order", () => {
    expect(usableCues([])).toEqual([]);
  });
});
