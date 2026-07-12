import { useEffect, useRef, useState } from "react";
import { getSongSlides, projectSlide, projectVerse, type Slide } from "../api";
import { type Cue, useLiveStore, useServiceStore } from "../services";

function isTypingTarget(el: EventTarget | null): boolean {
  const tag = (el as HTMLElement | null)?.tagName;
  return tag === "INPUT" || tag === "TEXTAREA";
}

export function ServicePanel() {
  const { cues, remove, move, clear } = useServiceStore();
  const [item, setItem] = useState(-1); // current cue index (-1 = none live)
  const [slide, setSlide] = useState(0); // slide index within a song cue
  const [slides, setSlides] = useState<Slide[]>([]);

  // Refs for the global key handler.
  const st = useRef({ cues, item, slide, slides });
  st.current = { cues, item, slide, slides };

  async function loadSlidesFor(cue: Cue): Promise<Slide[]> {
    if (cue.type !== "song") return [];
    const s = await getSongSlides(cue.songId);
    setSlides(s);
    return s;
  }

  async function projectItem(index: number, slideIdx = 0): Promise<void> {
    const cue = cues[index];
    if (!cue) return;
    useLiveStore.getState().setOwner("service");
    setItem(index);
    if (cue.type === "verse") {
      setSlides([]);
      setSlide(0);
      await projectVerse(cue.verse);
    } else {
      const s = st.current.slides.length && st.current.item === index ? st.current.slides : await loadSlidesFor(cue);
      const clamped = Math.min(Math.max(0, slideIdx), Math.max(0, s.length - 1));
      setSlide(clamped);
      if (s.length) await projectSlide(cue.songId, clamped);
    }
  }

  async function next(): Promise<void> {
    const { cues, item, slide, slides } = st.current;
    if (item < 0) return projectItem(0);
    const cue = cues[item];
    if (cue?.type === "song" && slide < slides.length - 1) {
      return projectItem(item, slide + 1);
    }
    if (item < cues.length - 1) return projectItem(item + 1);
  }

  async function prev(): Promise<void> {
    const { item, slide, cues } = st.current;
    if (item < 0) return;
    const cue = cues[item];
    if (cue?.type === "song" && slide > 0) return projectItem(item, slide - 1);
    if (item > 0) return projectItem(item - 1);
  }

  useEffect(() => {
    function onKey(e: KeyboardEvent): void {
      if (isTypingTarget(e.target)) return;
      if (useLiveStore.getState().owner !== "service") return;
      if (["ArrowRight", "ArrowDown", "PageDown", " "].includes(e.key)) {
        e.preventDefault();
        void next();
      } else if (["ArrowLeft", "ArrowUp", "PageUp"].includes(e.key)) {
        e.preventDefault();
        void prev();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const nextCue = cues[item < 0 ? 0 : item + 1];
  const cueLabel = (c: Cue): string =>
    c.type === "verse" ? `📖 ${c.verse.reference}` : `🎵 ${c.title}`;

  return (
    <section className="space-y-2">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <h2 className="panel-title">Service order</h2>
        <span className="text-xs text-[var(--faint)]">arrow keys step · songs advance slide by slide</span>
        {cues.length > 0 && (
          <button onClick={() => { clear(); setItem(-1); }} className="btn btn-sm btn-danger ml-auto">
            Clear
          </button>
        )}
      </div>

      {cues.length > 0 && nextCue && (
        <p className="text-xs text-gray-500">Next up: {cueLabel(nextCue)}</p>
      )}

      {cues.length === 0 ? (
        <p className="text-sm text-gray-400">
          Add verses (from Scripture) and songs (from Songs) to build a run order.
        </p>
      ) : (
        <ol className="space-y-1">
          {cues.map((cue, i) => (
            <li key={cue.id} className="flex items-center gap-2">
              <button
                onClick={() => projectItem(i)}
                className={`flex-1 rounded border px-2 py-1 text-left ${
                  i === item ? "border-green-600 bg-green-50" : "hover:bg-gray-50"
                }`}
              >
                <span className="mr-2 text-xs text-gray-400">{i + 1}</span>
                {cue.type === "verse" ? (
                  <span>📖 {cue.verse.reference}</span>
                ) : (
                  <span>
                    🎵 {cue.title}
                    {i === item && slides.length ? (
                      <span className="text-gray-500"> — slide {slide + 1}/{slides.length}</span>
                    ) : null}
                  </span>
                )}
              </button>
              <button onClick={() => move(cue.id, -1)} className="icon-btn" title="Move up">↑</button>
              <button onClick={() => move(cue.id, 1)} className="icon-btn" title="Move down">↓</button>
              <button onClick={() => remove(cue.id)} className="icon-btn" style={{ color: "var(--danger)" }} title="Remove">✕</button>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
