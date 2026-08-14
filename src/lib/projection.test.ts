import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { coversScreen, needsAssetUrl } from "./projection";

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

describe("needsAssetUrl", () => {
  it("converts the file paths media now arrives as", () => {
    // Deck pages are written to disk and referenced by absolute path. A webview
    // cannot load one directly, which is why a page previewed perfectly and
    // projected as a blank screen.
    expect(needsAssetUrl("C:\\Users\\a\\AppData\\Roaming\\app\\slides\\deck\\page-001.png")).toBe(true);
    expect(needsAssetUrl("D:/church/media/bumper.mp4")).toBe(true);
  });

  it("leaves alone what a webview can already load", () => {
    expect(needsAssetUrl("data:image/png;base64,iVBOR")).toBe(false);
    expect(needsAssetUrl("blob:http://localhost/abc")).toBe(false);
    expect(needsAssetUrl("https://example.org/a.png")).toBe(false);
    expect(needsAssetUrl("asset://localhost/a.png")).toBe(false);
    expect(needsAssetUrl("/newbreed_logo.png")).toBe(false);
  });

  it("does not try to convert nothing", () => {
    expect(needsAssetUrl("")).toBe(false);
    expect(needsAssetUrl("   ")).toBe(false);
  });
});

describe("both windows load media the same way", () => {
  it("neither passes a raw src straight to an element", () => {
    // The bug this pins: the projection window kept passing state.src directly
    // while the preview pane converted it, so the two disagreed about the same
    // file and only one of them showed it.
    const view = readFileSync(new URL("../ProjectionView.tsx", import.meta.url), "utf8");
    const pane = readFileSync(new URL("../components/PreviewPane.tsx", import.meta.url), "utf8");
    expect(view).not.toContain("src={state.src}");
    expect(view).toContain("needsAssetUrl");
    expect(pane).toContain("needsAssetUrl");
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
