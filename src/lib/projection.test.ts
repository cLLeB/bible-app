import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { coversScreen } from "./projection";

describe("coversScreen", () => {
  it("is true for the kinds that fill the screen with a picture", () => {
    expect(coversScreen({ kind: "image", src: "/m/a.png" })).toBe(true);
    expect(
      coversScreen({
        kind: "video",
        src: "/m/b.mp4",
        title: "B",
        paused: false,
        muted: false,
        looping: false,
      }),
    ).toBe(true);
  });

  it("is false for the kinds that are words", () => {
    expect(coversScreen({ kind: "verse", text: "t", caption: "c" })).toBe(false);
    expect(coversScreen({ kind: "song", text: "t", caption: "c" })).toBe(false);
    expect(coversScreen({ kind: "message", text: "t" })).toBe(false);
    expect(coversScreen({ kind: "countdown", targetMs: 0, label: "l" })).toBe(false);
    expect(coversScreen({ kind: "blank" })).toBe(false);
    expect(coversScreen({ kind: "blackout" })).toBe(false);
    expect(coversScreen({ kind: "logo" })).toBe(false);
  });
});

describe("the projection window's content wrapper", () => {
  const view = readFileSync(new URL("../ProjectionView.tsx", import.meta.url), "utf8");

  it("stretches for full-screen media instead of being sized by it", () => {
    // The bug this pins: images and videos position themselves with
    // `absolute inset-0`, which contributes no size. Inside a wrapper sized by
    // its content, that wrapper collapsed to zero and the item projected as
    // nothing, while verses were unaffected because text has its own size.
    expect(view).toContain("coversScreen");
    expect(view, "the wrapper must be positioned explicitly, not by class order")
      .toContain('position: "absolute", inset: 0');
  });

  it("asks the shared helper rather than re-listing the kinds", () => {
    // Two lists of "which kinds are pictures" drift, and the drift is invisible
    // until something projects blank in front of a congregation.
    expect(view).not.toMatch(/kind === "video" \|\| .*kind === "image"/);
  });
});
