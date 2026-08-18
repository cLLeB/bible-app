/**
 * One search box for everything an operator might reach for mid-service.
 *
 * Until now scripture, songs and media each had their own box in their own panel,
 * so finding something meant knowing which kind of thing it was first. During a
 * service that is exactly the wrong question to have to answer: the preacher says a
 * word and the operator wants whatever matches it, wherever it lives.
 *
 * The ranking rules live here, apart from the UI, because they are the part worth
 * being sure about.
 */

/** What kind of thing a hit is. */
export type HitKind = "reference" | "verse" | "song" | "media";

export interface Hit {
  kind: HitKind;
  /** Stable within a kind; used as the React key and to act on the hit. */
  id: string;
  title: string;
  /** Second line, when there is more worth showing. */
  detail?: string;
}

/** Does `title` match everything the operator has typed, in any order? */
export function titleMatches(title: string, query: string): boolean {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return false;
  const hay = title.toLowerCase();
  return words.every((w) => hay.includes(w));
}

/**
 * Does this look like someone typing a reference rather than searching for words?
 *
 * Deliberately loose: "john 3", "1 cor 13:4", "psalm 23" all count. It only decides
 * whether to *offer* a "go to" hit at the top; the backend parser has the final say
 * on whether it resolves, so a false positive here costs nothing but a row that
 * quietly fails to appear.
 */
export function looksLikeReference(query: string): boolean {
  return /^\s*\d?\s*[a-z]{2,}\s*\.?\s*\d+(\s*[:\-–]\s*\d+)?\s*$/i.test(query.trim());
}

/**
 * Order the hits.
 *
 * A typed reference comes first, because someone who types "John 3:16" wants that
 * verse and not a discussion of it. Songs before verses after that: an operator
 * searching mid-service is usually looking for the next item in the order, and a
 * song title is a much more deliberate thing to type than a word that happens to
 * appear in scripture. Media last, being the rarest thing to hunt for by name.
 */
const ORDER: HitKind[] = ["reference", "song", "verse", "media"];

export function rank(hits: Hit[]): Hit[] {
  return [...hits].sort((a, b) => ORDER.indexOf(a.kind) - ORDER.indexOf(b.kind));
}

/** Cap each kind so one sort of thing cannot bury the others. */
export function capPerKind(hits: Hit[], cap: number): Hit[] {
  const seen = new Map<HitKind, number>();
  return hits.filter((h) => {
    const n = (seen.get(h.kind) ?? 0) + 1;
    seen.set(h.kind, n);
    return n <= cap;
  });
}

/** Too short to be worth searching for. One letter matches half the library. */
export function worthSearching(query: string): boolean {
  return query.trim().length >= 2;
}
