/**
 * One rule for getting something onto the wall.
 *
 * The console grew two habits at once. Some things projected the instant they were
 * clicked; others staged into the preview and waited. Both are defensible and having
 * both is not: an operator who is unsure which a given list does will hesitate over
 * every click, and hesitating is the thing that shows.
 *
 * The rule, taken from the platform that handles it best:
 *
 *     click        →  preview it
 *     double click →  put it live
 *     Enter        →  put the previewed thing live
 *
 * Preview is free, so a single click can never embarrass anyone. Going live takes a
 * deliberate second action. The panic buttons (Blank, Blackout, Logo) are exempt and
 * always immediate, because the whole point of them is that they are instant.
 */

/** How long after a click a second one still counts as a double. */
const DOUBLE_MS = 400;

export interface Promoter {
  /** Call on every click. Returns what the operator meant. */
  click: (id: string, now?: number) => "preview" | "live";
  /** Forget any pending click, e.g. when the list changes underneath. */
  reset: () => void;
}

/**
 * Tracks clicks so a component can tell a preview from a promotion.
 *
 * Keyed by item id rather than by time alone: clicking two *different* rows quickly
 * is browsing, not a double click, and treating it as one would put the wrong thing
 * live. That is the failure this exists to avoid.
 */
export function makePromoter(doubleMs: number = DOUBLE_MS): Promoter {
  let lastId: string | null = null;
  let lastAt = 0;
  return {
    click(id, now = Date.now()) {
      const isDouble = id === lastId && now - lastAt <= doubleMs;
      // A double click ends the run: a third click starts a fresh preview rather
      // than firing live again.
      lastId = isDouble ? null : id;
      lastAt = isDouble ? 0 : now;
      return isDouble ? "live" : "preview";
    },
    reset() {
      lastId = null;
      lastAt = 0;
    },
  };
}
