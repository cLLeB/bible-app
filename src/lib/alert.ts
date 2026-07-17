import type { Alert } from "../api";

/**
 * Whether a lower-third alert should currently show. An alert is visible when it
 * has text and either never expires (`untilMs === 0`) or its dismiss time is
 * still in the future. Pure so the projection window's expiry logic is testable.
 */
export function alertVisible(alert: Alert, now: number): boolean {
  if (alert.text.trim() === "") return false;
  return alert.untilMs === 0 || now < alert.untilMs;
}
