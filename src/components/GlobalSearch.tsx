import { useCallback, useEffect, useRef, useState } from "react";
import {
  listMedia,
  listSongs,
  lookupReference,
  projectMedia,
  projectSlide,
  searchScripture,
  type MediaLibraryItem,
  type SongSummary,
} from "../api";
import { present } from "../present";
import {
  capPerKind,
  looksLikeReference,
  rank,
  titleMatches,
  worthSearching,
  type Hit,
} from "../lib/search";
import { useServiceStore } from "../services";

/** How many of each kind to show, so one sort cannot bury the others. */
const CAP = 4;

/**
 * One box for scripture, songs and media.
 *
 * Scripture and songs each had their own search, in their own panel, which meant
 * finding something required knowing what kind of thing it was first. Mid-service
 * that is the wrong question: the preacher says a word and the operator wants
 * whatever matches, wherever it lives.
 *
 * Songs and media are filtered here rather than in the database because both
 * libraries are small enough to hold in memory and the lists are already loaded for
 * their own panels. Scripture is not, so its text search stays where it belongs.
 */
export function GlobalSearch() {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<Hit[]>([]);
  const [busy, setBusy] = useState(false);
  const addVerse = useServiceStore((s) => s.addVerse);

  // The two small libraries, kept to hand so typing does not hit the backend for
  // every keystroke.
  const songs = useRef<SongSummary[]>([]);
  const media = useRef<MediaLibraryItem[]>([]);
  useEffect(() => {
    void listSongs().then((s) => (songs.current = s)).catch(() => undefined);
    void listMedia().then((m) => (media.current = m)).catch(() => undefined);
  }, []);

  const run = useCallback(async (q: string): Promise<void> => {
    if (!worthSearching(q)) {
      setHits([]);
      return;
    }
    const found: Hit[] = [];

    if (looksLikeReference(q)) {
      found.push({ kind: "reference", id: `ref:${q}`, title: `Go to ${q.trim()}` });
    }
    for (const s of songs.current) {
      if (titleMatches(s.title, q)) {
        found.push({ kind: "song", id: `song:${s.id}`, title: s.title, detail: s.author ?? undefined });
      }
    }
    for (const m of media.current) {
      if (titleMatches(m.title, q)) {
        found.push({ kind: "media", id: `media:${m.id}`, title: m.title, detail: m.kind });
      }
    }
    setBusy(true);
    try {
      for (const v of await searchScripture(q)) {
        found.push({
          kind: "verse",
          id: `verse:${v.bookOsis}:${v.chapter}:${v.verse}`,
          title: v.reference,
          detail: v.text,
        });
      }
    } catch {
      /* scripture search is one source of several; the rest still stand */
    } finally {
      setBusy(false);
    }
    setHits(capPerKind(rank(found), CAP));
  }, []);

  // Debounced, because scripture search is a database query and an operator types
  // faster than one finishes.
  useEffect(() => {
    const t = setTimeout(() => void run(query), 180);
    return () => clearTimeout(t);
  }, [query, run]);

  async function choose(h: Hit): Promise<void> {
    try {
      if (h.kind === "reference" || h.kind === "verse") {
        const ref = h.kind === "reference" ? query.trim() : h.title;
        const v = await lookupReference(ref);
        await present(v);
        addVerse(v);
      } else if (h.kind === "media") {
        await projectMedia(Number(h.id.split(":")[1]));
      } else if (h.kind === "song") {
        // From the top. Any other slide would be a guess about where the operator
        // means to come in, and the first one is the only answer that is never
        // surprising - they can step forward from there in a keypress.
        await projectSlide(Number(h.id.split(":")[1]), 0);
      }
      setQuery("");
      setHits([]);
    } catch {
      /* a hit that will not open is not worth an alert mid-service */
    }
  }

  return (
    <div className="space-y-1.5">
      <input
        className="input w-full"
        placeholder="Search scripture, songs, media"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            setQuery("");
            setHits([]);
          }
          if (e.key === "Enter" && hits.length > 0) void choose(hits[0]);
        }}
      />
      {hits.length > 0 && (
        <ul className="rounded border" style={{ borderColor: "var(--border)" }}>
          {hits.map((h) => (
            <li key={h.id}>
              <button
                className="flex w-full items-baseline gap-2 px-2 py-1 text-left text-sm hover:bg-[var(--surface)]"
                onClick={() => void choose(h)}
              >
                <span className="text-xs uppercase tracking-wide text-[var(--faint)]">
                  {h.kind === "reference" ? "go" : h.kind}
                </span>
                <span>{h.title}</span>
                {h.detail && (
                  <span className="truncate text-xs text-[var(--muted)]">{h.detail}</span>
                )}
              </button>
            </li>
          ))}
        </ul>
      )}
      {busy && hits.length === 0 && (
        <div className="text-xs text-[var(--faint)]">Searching…</div>
      )}
    </div>
  );
}
