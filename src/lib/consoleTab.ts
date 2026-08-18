/**
 * Which part of the console is showing.
 *
 * Three rather than two, because "prepare" was carrying two different jobs: things
 * done every week for this Sunday (the songs, the media, the running order) and
 * things done once when the machine was set up (the sound input, the screens, the
 * voice calibration). Mixing them meant a first-timer met all of it at once, and
 * meant setup controls sat on the surface an operator uses mid-service.
 */
export type ConsoleTab = "live" | "plan" | "setup";

const KEY = "console-tab";
const TABS: readonly ConsoleTab[] = ["live", "plan", "setup"];

function isTab(value: string | null): value is ConsoleTab {
  return value !== null && (TABS as readonly string[]).includes(value);
}

/**
 * The tab the console last showed. Anything unrecognised falls back to Live: a
 * value from an older build, a hand-edited key, or storage that refuses to
 * answer. A service is the case where landing on the wrong screen actually costs
 * something.
 */
export function loadTab(): ConsoleTab {
  try {
    const stored = localStorage.getItem(KEY);
    // "prepare" was split into plan and setup; anyone who left the console there
    // lands on the half they were far more likely to have been using.
    if (stored === "prepare") return "plan";
    return isTab(stored) ? stored : "live";
  } catch {
    return "live";
  }
}

export function saveTab(tab: ConsoleTab): void {
  try {
    localStorage.setItem(KEY, tab);
  } catch {
    /* storage unavailable; the tab just won't persist, which is harmless */
  }
}
