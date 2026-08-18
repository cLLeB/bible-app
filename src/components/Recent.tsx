import { useRef } from "react";
import { projectMedia, projectSlide } from "../api";
import { present } from "../present";
import { makePromoter } from "../lib/promote";
import {
  usePreviewStore,
  useRecentStore,
  useServiceStore,
  type RecentItem,
} from "../services";
import { verseSlot } from "../stage";

/**
 * What was on the wall a moment ago, of any kind, with a way to put each one back
 * or onto the running order.
 *
 * It used to list verses only, which made "Recent" quietly mean "recent scripture".
 * An operator who had just shown a song and a slide saw neither and had to go back
 * to the panel they came from - and going back to the panel is the expensive part
 * mid-service.
 *
 * Follows the console's rule for reaching the wall: a click previews, a second click
 * on the same item goes live. The + is the other half, adding to the running order
 * without touching the screen.
 */
export function Recent() {
  const items = useRecentStore((s) => s.items);
  const stage = usePreviewStore((s) => s.stage);
  const addVerse = useServiceStore((s) => s.addVerse);
  const addSong = useServiceStore((s) => s.addSong);
  const addMedia = useServiceStore((s) => s.addMedia);
  const promoter = useRef(makePromoter());

  if (items.length === 0) return null;

  function open(item: RecentItem, live: boolean): void {
    if (item.kind === "verse") {
      if (live) {
        void present(item.verse);
      } else {
        stage({
          kind: "verse",
          text: item.verse.text,
          caption: verseSlot(item.verse).caption,
        });
      }
      return;
    }
    // Songs and media have no preview representation of their own here - a song is a
    // list of slides and a picture is a file - so a click opens them. Verses are the
    // thing an operator flicks through, and the thing worth staging.
    if (item.kind === "song") {
      void projectSlide(item.songId, 0).catch(() => undefined);
    } else {
      void projectMedia(item.mediaId).catch(() => undefined);
    }
  }

  function add(item: RecentItem): void {
    if (item.kind === "verse") addVerse(item.verse);
    else if (item.kind === "song") addSong(item.songId, item.title);
    else addMedia(item.mediaId, item.title, item.mediaKind);
  }

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <span className="panel-title">Recent</span>
      {items.map((item) => (
        <span key={item.id} className="inline-flex items-center">
          <button
            onClick={() => open(item, promoter.current.click(item.id) === "live")}
            className="btn btn-sm"
            title={item.kind === "verse" ? item.verse.text : item.title}
          >
            {item.kind !== "verse" && (
              <span className="mr-1 text-[var(--faint)]">{item.kind === "song" ? "♪" : "▣"}</span>
            )}
            {item.title}
          </button>
          <button
            onClick={() => add(item)}
            className="btn btn-sm px-1.5"
            aria-label={`Add ${item.title} to the running order`}
            title="Add to the running order"
          >
            +
          </button>
        </span>
      ))}
    </div>
  );
}
