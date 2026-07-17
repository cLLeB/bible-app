import { describe, expect, it } from "vitest";
import type { StageTimer } from "../api";
import { stageTimerElapsed, stageTimerText } from "./stageTimer";

describe("stageTimerText", () => {
  it("is null when off", () => {
    expect(stageTimerText({ mode: "off", anchorMs: 0 }, 1000)).toBeNull();
  });
  it("counts up elapsed time as m:ss", () => {
    const t: StageTimer = { mode: "countup", anchorMs: 0 };
    expect(stageTimerText(t, 65_000)).toBe("1:05");
  });
  it("counts down remaining time and floors at 0:00", () => {
    const t: StageTimer = { mode: "countdown", anchorMs: 300_000 };
    expect(stageTimerText(t, 0)).toBe("5:00");
    expect(stageTimerText(t, 299_000)).toBe("0:01");
    expect(stageTimerText(t, 400_000)).toBe("0:00");
  });
  it("shows hours when over an hour", () => {
    expect(stageTimerText({ mode: "countup", anchorMs: 0 }, 3_661_000)).toBe("1:01:01");
  });
});

describe("stageTimerElapsed", () => {
  it("is true once a countdown passes zero", () => {
    const t: StageTimer = { mode: "countdown", anchorMs: 1000 };
    expect(stageTimerElapsed(t, 999)).toBe(false);
    expect(stageTimerElapsed(t, 1000)).toBe(true);
  });
  it("is false for count-up and off", () => {
    expect(stageTimerElapsed({ mode: "countup", anchorMs: 0 }, 99999)).toBe(false);
    expect(stageTimerElapsed({ mode: "off", anchorMs: 0 }, 99999)).toBe(false);
  });
});
