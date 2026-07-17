import { describe, expect, it } from "vitest";
import type { Alert } from "../api";
import { alertVisible } from "./alert";

describe("alertVisible", () => {
  it("is hidden when there is no text", () => {
    expect(alertVisible({ text: "", untilMs: 0 }, 100)).toBe(false);
    expect(alertVisible({ text: "   ", untilMs: 0 }, 100)).toBe(false);
  });
  it("shows a sticky alert (untilMs 0) regardless of time", () => {
    expect(alertVisible({ text: "Nursery: room 3", untilMs: 0 }, 999999)).toBe(true);
  });
  it("shows a timed alert only before its dismiss time", () => {
    const a: Alert = { text: "Fire drill", untilMs: 1000 };
    expect(alertVisible(a, 999)).toBe(true);
    expect(alertVisible(a, 1000)).toBe(false);
    expect(alertVisible(a, 1500)).toBe(false);
  });
});
