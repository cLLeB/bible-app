import type { StageTimer } from "../api";

function clock(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const mm = h > 0 ? m.toString().padStart(2, "0") : m.toString();
  const body = `${mm}:${sec.toString().padStart(2, "0")}`;
  return h > 0 ? `${h}:${body}` : body;
}

/**
 * The text for the stage timer, or null when it's off. Count-up shows elapsed
 * since the anchor; count-down shows time remaining to it (never negative).
 * Pure so the stage monitor's display is testable without a clock.
 */
export function stageTimerText(timer: StageTimer, now: number): string | null {
  if (timer.mode === "countup") return clock((now - timer.anchorMs) / 1000);
  if (timer.mode === "countdown") return clock((timer.anchorMs - now) / 1000);
  return null;
}

/** Whether a count-down has passed zero (for styling it as over-time). */
export function stageTimerElapsed(timer: StageTimer, now: number): boolean {
  return timer.mode === "countdown" && now >= timer.anchorMs;
}
