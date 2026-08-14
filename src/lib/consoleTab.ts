/** Which half of the console is showing: operating a service, or preparing one. */
export type ConsoleTab = "live" | "prepare";

const KEY = "console-tab";
const TABS: readonly ConsoleTab[] = ["live", "prepare"];

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
