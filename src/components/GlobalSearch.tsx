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
  type VersePayload,
} from "../api";
import { present } from "../present";
import { rememberMedia, rememberSong, useServiceStore } from "../services";
import {
  capPerKind,
  rank,
  titleMatches,
  worthSearching,
  type Hit,
} from "../lib/search";

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
  const [added, setAdded] = useState<string | null>(null);
  const addVerse = useServiceStore((st) => st.addVerse);
  const addSong = useServiceStore((st) => st.addSong);
  const addMedia = useServiceStore((st) => st.addMedia);

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

    // Whether this is a reference is the parser's question, not a regex's. It knows
    // the books, the abbreviations and the spoken forms; a pattern here got "John
    // 3:3" right and "John chapter 3 verse 3" wrong, which meant Enter fell through
    // to the text search and opened whatever verse happened to contain those words.
    const asReference = await lookupReference(q).catch(() => null);
    if (asReference) {
      found.push({
        kind: "reference",
        id: `ref:${asReference.bookOsis}:${asReference.chapter}:${asReference.verse}`,
        title: asReference.reference,
        detail: asReference.text,
        verse: asReference,
      });
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
        // A reference hit already holds the resolved verse, so clicking it cannot
        // re-parse the query and land somewhere else.
        // Projected, and nothing more. Putting a verse on the wall is not the same
        // as putting it in the running order: the order is a plan the operator built
        // on purpose, and a verse looked up mid-sermon because the preacher went
        // somewhere unexpected has no business appending itself to it. Every other
        // list in the console has a separate Add button for that, and this one was
        // the odd one out.
        const v = (h.verse as VersePayload | undefined) ?? (await lookupReference(h.title));
        await present(v);
      } else if (h.kind === "media") {
        const mediaId = Number(h.id.split(":")[1]);
        await projectMedia(mediaId);
        rememberMedia(mediaId, h.title, h.detail === "video" || h.detail === "audio" ? h.detail : "image");
      } else if (h.kind === "song") {
        // From the top. Any other slide would be a guess about where the operator
        // means to come in, and the first one is the only answer that is never
        // surprising - they can step forward from there in a keypress.
        const songId = Number(h.id.split(":")[1]);
        await projectSlide(songId, 0);
        rememberSong(songId, h.title);
      }
      setQuery("");
      setHits([]);
    } catch {
      /* a hit that will not open is not worth an alert mid-service */
    }
  }

  /**
   * Put a hit on the running order without projecting it.
   *
   * The deliberate half of what one click used to do by itself. Building the order
   * and putting something on the wall are separate decisions, so they are separate
   * buttons - which is how every other list in the console already worked.
   */
  function add(h: Hit): void {
    if (h.kind === "reference" || h.kind === "verse") {
      const v = h.verse as VersePayload | undefined;
      if (v) {
        addVerse(v);
      } else {
        void lookupReference(h.title).then(addVerse).catch(() => undefined);
      }
    } else if (h.kind === "song") {
      addSong(Number(h.id.split(":")[1]), h.title);
    } else if (h.kind === "media") {
      const kind = h.detail === "video" || h.detail === "audio" ? h.detail : "image";
      addMedia(Number(h.id.split(":")[1]), h.title, kind);
    }
    // Named rather than counted: "Added" on the row the operator just pressed is
    // the only confirmation needed, and it clears itself.
    setAdded(h.id);
    setTimeout(() => setAdded((cur) => (cur === h.id ? null : cur)), 1200);
  }

  return (
    <div className="space-y-1.5">
      <input
        className="input w-full"
        data-search
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
            <li key={h.id} className="flex items-center gap-1 pr-1">
              <button
                className="flex min-w-0 flex-1 items-baseline gap-2 px-2 py-1 text-left text-sm hover:bg-[var(--surface)]"
                onClick={() => void choose(h)}
              >
                <span className="text-xs uppercase tracking-wide text-[var(--faint)]">
                  {h.kind === "reference" ? "go" : h.kind}
                </span>
                <span className="flex-none">{h.title}</span>
                {h.detail && (
                  <span className="truncate text-xs text-[var(--muted)]">{h.detail}</span>
                )}
              </button>
              <button
                className="btn btn-sm flex-none"
                onClick={() => add(h)}
                aria-label={`Add ${h.title} to the running order`}
              >
                {added === h.id ? "Added" : "Add"}
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
