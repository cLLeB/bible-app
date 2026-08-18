import { useRef } from "react";
import { present } from "../present";
import { usePreviewStore, useScriptureStore } from "../services";
import { makePromoter } from "../lib/promote";
import { verseSlot } from "../stage";

/**
 * Verses shown a moment ago, for putting one straight back up.
 *
 * Follows the console's one rule for reaching the wall: a click previews, a second
 * click on the same one goes live. Clicking down the list is browsing and cannot
 * project anything by accident.
 */
export function RecentVerses() {
  const recents = useScriptureStore((s) => s.recents);
  const stage = usePreviewStore((s) => s.stage);
  const promoter = useRef(makePromoter());
  if (recents.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <span className="panel-title">Recent</span>
      {recents.map((v, i) => {
        const id = `${v.reference}-${v.translation}-${i}`;
        return (
          <button
            key={id}
            onClick={() => {
              if (promoter.current.click(id) === "live") {
                void present(v);
              } else {
                stage({ kind: "verse", text: v.text, caption: verseSlot(v).caption });
              }
            }}
            className="btn btn-sm"
            title={v.text}
          >
            {v.reference}
          </button>
        );
      })}
    </div>
  );
}
