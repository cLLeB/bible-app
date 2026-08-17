import { describe, expect, it } from "vitest";
import { namesHidden, resolveRemembered, type SoundOutput } from "./audioSink";

const tv: SoundOutput = { id: "abc123", label: "Samsung TV (NVIDIA High Definition Audio)" };
const laptop: SoundOutput = { id: "def456", label: "Speakers (Realtek(R) Audio)" };

describe("remembering which speakers to use", () => {
  it("uses the remembered device when it is still plugged in", () => {
    expect(resolveRemembered([tv, laptop], tv)).toBe("abc123");
  });

  it("finds the same TV again after its id has changed", () => {
    // Device ids are per-install and do not always survive a reinstall. The name
    // does, and a TV that comes back as a new id is still plainly the same TV.
    const renumbered = { ...tv, id: "brand-new-id" };
    expect(resolveRemembered([renumbered, laptop], tv)).toBe("brand-new-id");
  });

  it("falls back to the system default when the device is genuinely gone", () => {
    // The TV is unplugged. Sound out of the laptop is a poor outcome; a service
    // with no sound at all is a worse one.
    expect(resolveRemembered([laptop], tv)).toBe("");
  });

  it("treats nothing remembered as the system default", () => {
    expect(resolveRemembered([tv, laptop], { id: "", label: "" })).toBe("");
  });

  it("does not match a different device that happens to be listed first", () => {
    expect(resolveRemembered([laptop], { id: "gone", label: "Some Other TV" })).toBe("");
  });
});

describe("whether the outputs still need naming", () => {
  it("spots the placeholder names the browser leaves behind", () => {
    // Without microphone permission the labels come back empty and are filled in
    // with placeholders, which are no use to an operator choosing between them.
    expect(namesHidden([{ id: "a", label: "Sound output 1" }])).toBe(true);
  });

  it("is satisfied once real names are available", () => {
    expect(namesHidden([tv, laptop])).toBe(false);
  });

  it("does not count the system default entry as unnamed", () => {
    expect(namesHidden([{ id: "default", label: "System default" }, tv])).toBe(false);
  });
});
