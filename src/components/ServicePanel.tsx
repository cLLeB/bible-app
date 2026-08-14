import { useEffect, useRef, useState } from "react";
import {
  getSongSlides,
  presentCoords,
  projectMedia,
  projectSlide,
  projectVerse,
  setStage,
  type Slide,
  type StageSlot,
} from "../api";
import {
  type Cue,
  useLiveStore,
  useScriptureStore,
  useServiceStore,
  useTemplateStore,
} from "../services";
import { slideSlot, verseSlot } from "../stage";

function isTypingTarget(el: EventTarget | null): boolean {
  const tag = (el as HTMLElement | null)?.tagName;
  return tag === "INPUT" || tag === "TEXTAREA";
}

export function ServicePanel() {
  const { cues, remove, move, clear, setCues } = useServiceStore();
  const { templates, save: saveTemplate, remove: removeTemplate } = useTemplateStore();
  const [item, setItem] = useState(-1); // current cue index (-1 = none live)
  const [slide, setSlide] = useState(0); // slide index within a song cue
  const [slides, setSlides] = useState<Slide[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [templateName, setTemplateName] = useState("");
  const [chosenTemplate, setChosenTemplate] = useState("");

  // Refs for the global key handler.
  const st = useRef({ cues, item, slide, slides });
  st.current = { cues, item, slide, slides };

  async function loadSlidesFor(cue: Cue): Promise<Slide[]> {
    if (cue.type !== "song") return [];
    const s = await getSongSlides(cue.songId);
    setSlides(s);
    return s;
  }

  // The first line of the cue at `i`, for the stage "next" preview.
  async function cueSlot(i: number): Promise<StageSlot | null> {
    const cue = st.current.cues[i];
    if (!cue) return null;
    if (cue.type === "verse") return verseSlot(cue.verse);
    // Media has no lyrics to preview, so the platform team gets its name and
    // kind — enough to know a video is coming and not to start singing.
    if (cue.type === "media") {
      return { text: cue.title, caption: cue.kind === "video" ? "Video" : "Image" };
    }
    const s = await getSongSlides(cue.songId).catch(() => [] as Slide[]);
    return slideSlot(s[0]?.text ?? "", cue.title);
  }

  async function projectItem(index: number, slideIdx = 0): Promise<void> {
    // Read from the live ref, not the closure — the keyboard handler is bound
    // once and would otherwise see a stale (empty) cue list.
    const cue = st.current.cues[index];
    if (!cue) return;
    useLiveStore.getState().setOwner("service");
    setItem(index);
    setError(null);
    try {
      let current: StageSlot;
      let next: StageSlot | null;
      if (cue.type === "verse") {
        setSlides([]);
        setSlide(0);
        // Re-fetch by coordinates so it always projects fresh in the current
        // translation (robust to any stale stored payload). Fall back to the
        // stored payload if coordinates are unavailable.
        const v = cue.verse;
        const shown =
          v.bookOsis != null
            ? await presentCoords(v.bookOsis, v.chapter, v.verse)
            : (await projectVerse(v), v);
        useScriptureStore.getState().setCurrent(shown);
        current = verseSlot(shown);
        next = await cueSlot(index + 1);
      } else if (cue.type === "media") {
        setSlides([]);
        setSlide(0);
        // Project by library id so a renamed or re-pointed item still resolves,
        // and so a run order saved as a template keeps working.
        const shown = await projectMedia(cue.mediaId);
        current = { text: shown.title, caption: shown.kind === "video" ? "Video" : "Image" };
        next = await cueSlot(index + 1);
      } else {
        const s =
          st.current.slides.length && st.current.item === index
            ? st.current.slides
            : await loadSlidesFor(cue);
        const clamped = Math.min(Math.max(0, slideIdx), Math.max(0, s.length - 1));
        setSlide(clamped);
        if (s.length) await projectSlide(cue.songId, clamped);
        current = slideSlot(s[clamped]?.text ?? "", cue.title);
        // Next slide in this song, else the first line of the next cue.
        next =
          clamped + 1 < s.length
            ? slideSlot(s[clamped + 1].text, cue.title)
            : await cueSlot(index + 1);
      }
      void setStage(current, next).catch(() => {});
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
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
  const cueLabel = (c: Cue): string => {
    if (c.type === "verse") return `📖 ${c.verse.reference}`;
    if (c.type === "media") return `${c.kind === "video" ? "🎬" : "🖼"} ${c.title}`;
    return `🎵 ${c.title}`;
  };

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

      {error && <p className="tint tint-bad rounded px-2 py-1 text-sm">{error}</p>}

      {/* Templates: save the current order, or load/replace with a saved one. */}
      <div className="flex flex-wrap items-center gap-2 border-b pb-2 text-sm">
        <input
          value={templateName}
          onChange={(e) => setTemplateName(e.target.value)}
          placeholder="Template name"
          className="input w-40"
        />
        <button
          onClick={() => {
            if (templateName.trim() && cues.length) {
              saveTemplate(templateName.trim(), cues);
              setTemplateName("");
            }
          }}
          className="btn btn-sm"
          title="Save the current run order as a reusable template"
        >
          Save
        </button>
        {templates.length > 0 && (
          <>
            <select
              className="select"
              style={{ width: "auto" }}
              value={chosenTemplate}
              onChange={(e) => setChosenTemplate(e.target.value)}
            >
              <option value="">Load template…</option>
              {templates.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                </option>
              ))}
            </select>
            <button
              onClick={() => {
                const t = templates.find((x) => x.id === chosenTemplate);
                if (t) {
                  setCues(t.cues);
                  setItem(-1);
                }
              }}
              className="btn btn-sm"
              disabled={!chosenTemplate}
            >
              Load
            </button>
            <button
              onClick={() => {
                if (chosenTemplate) {
                  removeTemplate(chosenTemplate);
                  setChosenTemplate("");
                }
              }}
              className="btn btn-sm"
              disabled={!chosenTemplate}
              title="Delete the selected template"
            >
              ✕
            </button>
          </>
        )}
      </div>

      {cues.length > 0 && nextCue && (
        <p className="text-xs text-[var(--muted)]">Next up: {cueLabel(nextCue)}</p>
      )}

      {cues.length === 0 ? (
        <p className="text-sm text-[var(--faint)]">
          Add verses (from Scripture) and songs (from Songs) to build a run order.
        </p>
      ) : (
        <ol className="space-y-1">
          {cues.map((cue, i) => (
            <li key={cue.id} className="flex items-center gap-2">
              <button
                onClick={() => projectItem(i)}
                className={`flex-1 rounded border px-2 py-1 text-left ${
                  i === item ? "tint tint-current" : "tint-neutral tint-hover"
                }`}
              >
                <span className="mr-2 text-xs text-[var(--faint)]">{i + 1}</span>
                {cue.type === "song" ? (
                  <span>
                    🎵 {cue.title}
                    {i === item && slides.length ? (
                      <span className="text-gray-500"> · slide {slide + 1}/{slides.length}</span>
                    ) : null}
                  </span>
                ) : (
                  <span>{cueLabel(cue)}</span>
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
