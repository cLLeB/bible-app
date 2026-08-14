import { describe, expect, it } from "vitest";
import {
  clampInterval,
  mediaKind,
  nextIndex,
  SLIDESHOW_DEFAULT_SECONDS,
  SLIDESHOW_MAX_SECONDS,
  SLIDESHOW_MIN_SECONDS,
  titleFromPath,
} from "./media";

describe("mediaKind", () => {
  it("recognises images and videos regardless of case", () => {
    expect(mediaKind("C:/media/backdrop.JPG")).toBe("image");
    expect(mediaKind("/home/a/loop.webm")).toBe("video");
    expect(mediaKind("welcome.PNG")).toBe("image");
    expect(mediaKind("bumper.MP4")).toBe("video");
  });

  it("refuses what the projection window cannot show", () => {
    expect(mediaKind("notes.pdf")).toBeNull();
    expect(mediaKind("song.mp3")).toBeNull();
    expect(mediaKind("README")).toBeNull();
    expect(mediaKind("archive.tar.gz")).toBeNull();
  });
});

describe("titleFromPath", () => {
  it("takes the file's own name, without folders or extension", () => {
    expect(titleFromPath("C:\\church\\media\\Advent Week 1.mp4")).toBe("Advent Week 1");
    expect(titleFromPath("/srv/media/offering.png")).toBe("offering");
  });

  it("keeps something usable for odd names", () => {
    expect(titleFromPath("LICENSE")).toBe("LICENSE");
    expect(titleFromPath(".hidden")).toBe(".hidden");
  });
});

describe("clampInterval", () => {
  it("keeps sensible values", () => {
    expect(clampInterval(8)).toBe(8);
    expect(clampInterval("15")).toBe(15);
    expect(clampInterval(7.4)).toBe(7);
  });

  it("falls back rather than throwing on an empty or nonsense box", () => {
    // Mid-service, a typo must not be able to stop the show.
    expect(clampInterval("")).toBe(SLIDESHOW_DEFAULT_SECONDS);
    expect(clampInterval("abc")).toBe(SLIDESHOW_DEFAULT_SECONDS);
    expect(clampInterval(-4)).toBe(SLIDESHOW_DEFAULT_SECONDS);
  });

  it("holds the floor and ceiling", () => {
    // A mistyped 0 would otherwise flash the whole library past the congregation.
    expect(clampInterval(0.2)).toBe(SLIDESHOW_MIN_SECONDS);
    expect(clampInterval(99999)).toBe(SLIDESHOW_MAX_SECONDS);
  });
});

describe("nextIndex", () => {
  it("walks forward and stops at the end", () => {
    expect(nextIndex(0, 3, false)).toBe(1);
    expect(nextIndex(1, 3, false)).toBe(2);
    expect(nextIndex(2, 3, false)).toBeNull();
  });

  it("wraps when looping, so an announcements loop can be left running", () => {
    expect(nextIndex(2, 3, true)).toBe(0);
  });

  it("has nowhere to go in an empty library", () => {
    expect(nextIndex(0, 0, true)).toBeNull();
    expect(nextIndex(0, 0, false)).toBeNull();
  });
});
