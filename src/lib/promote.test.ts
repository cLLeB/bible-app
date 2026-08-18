import { describe, expect, it } from "vitest";
import { makePromoter } from "./promote";

describe("telling a preview from a promotion", () => {
  it("previews on a single click", () => {
    const p = makePromoter();
    expect(p.click("a", 1000)).toBe("preview");
  });

  it("goes live on a second click of the same thing", () => {
    const p = makePromoter(400);
    expect(p.click("a", 1000)).toBe("preview");
    expect(p.click("a", 1200)).toBe("live");
  });

  it("treats two different rows in quick succession as browsing", () => {
    // The failure this exists to avoid: clicking down a list fast must never put
    // the wrong thing on the wall.
    const p = makePromoter(400);
    expect(p.click("a", 1000)).toBe("preview");
    expect(p.click("b", 1100)).toBe("preview");
    expect(p.click("c", 1200)).toBe("preview");
  });

  it("is a fresh preview once the pause is long enough", () => {
    const p = makePromoter(400);
    expect(p.click("a", 1000)).toBe("preview");
    expect(p.click("a", 1500)).toBe("preview");
  });

  it("does not fire live again on a third click", () => {
    // Otherwise a hesitant triple-click re-projects, which reads as a flicker.
    const p = makePromoter(400);
    p.click("a", 1000);
    expect(p.click("a", 1100)).toBe("live");
    expect(p.click("a", 1200)).toBe("preview");
  });

  it("forgets a pending click when told to", () => {
    // The list changed underneath: whatever was half-clicked is no longer the same
    // row, and a second click must not promote something the operator never saw.
    const p = makePromoter(400);
    p.click("a", 1000);
    p.reset();
    expect(p.click("a", 1100)).toBe("preview");
  });
});
