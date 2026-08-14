import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { ProjectionState } from "../api";
import { previewLabel, previewLines } from "./preview";

describe("previewLines", () => {
  it("shows words for the text kinds", () => {
    expect(previewLines({ kind: "verse", text: "For God so loved", caption: "John 3:16" }))
      .toEqual({ body: "For God so loved", caption: "John 3:16", visual: false });
    expect(previewLines({ kind: "song", text: "Amazing grace", caption: "Amazing Grace" }).body)
      .toBe("Amazing grace");
  });

  it("shows both columns and both codes for a comparison", () => {
    const lines = previewLines({
      kind: "parallel",
      primaryText: "one",
      primaryCode: "WEB",
      secondaryText: "two",
      secondaryCode: "KJV",
      caption: "John 3:16",
    });
    expect(lines.body).toBe("one\n\ntwo");
    expect(lines.caption).toBe("John 3:16 (WEB / KJV)");
  });

  it("marks pictures as visual so the pane shows the file, not a text block", () => {
    expect(previewLines({ kind: "image", src: "/m/a.png" }).visual).toBe(true);
    const video = previewLines({
      kind: "video",
      src: "/m/b.mp4",
      title: "Bumper",
      paused: false,
      muted: false,
      looping: false,
    });
    expect(video.visual).toBe(true);
    expect(video.caption).toBe("Video · Bumper");
  });

  it("names the states that show nothing, rather than previewing as blank", () => {
    // A pane that looks empty for blackout and empty for "not staged yet" tells
    // the operator nothing at the moment they most need telling.
    expect(previewLines({ kind: "blackout" }).caption).toBe("Blackout");
    expect(previewLines({ kind: "blank" }).caption).toBe("Blank");
    expect(previewLines({ kind: "logo" }).caption).toBe("Logo");
  });

  it("covers every kind the projection state can be", () => {
    // The pane is a promise about what the wall will show. A kind added to the
    // union without a branch here would quietly preview as "Blank".
    const api = readFileSync(new URL("../api.ts", import.meta.url), "utf8");
    const union = api.slice(
      api.indexOf("export type ProjectionState ="),
      api.indexOf("/** Where the congregation screen's background comes from"),
    );
    const kinds = [...union.matchAll(/kind:\s*"([a-z]+)"/g)].map((m) => m[1]);
    expect(kinds.length).toBeGreaterThan(5);
    const source = readFileSync(new URL("./preview.ts", import.meta.url), "utf8");
    for (const kind of kinds) {
      expect(source, `previewLines has no branch for "${kind}"`).toContain(`case "${kind}"`);
    }
  });
});

describe("previewLabel", () => {
  it("prefers the caption", () => {
    expect(previewLabel({ kind: "verse", text: "x", caption: "John 3:16" })).toBe("John 3:16");
  });

  it("falls back to the first line, trimmed to fit a button", () => {
    const long = "a".repeat(80);
    expect(previewLabel({ kind: "message", text: long })).toBe(`${"a".repeat(40)}…`);
    expect(previewLabel({ kind: "message", text: "Welcome\nsecond" })).toBe("Welcome");
  });

  it("says something rather than nothing when there is no content", () => {
    expect(previewLabel({ kind: "message", text: "" } as ProjectionState)).toBe("Nothing");
  });
});
