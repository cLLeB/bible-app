import { useEffect, useRef, useState } from "react";
import {
  blankProjection,
  getSong,
  getSongSlides,
  projectSlide,
  updateSong,
  type Slide,
  type SongSummary,
} from "../api";
import { blockAt, replaceBlock, sections, shortLabel } from "../lib/sections";
import { rememberSong, useLiveStore } from "../services";
import { clearStage, slideSlot } from "../stage";
import { setStage } from "../api";

interface SongLiveProps {
  song: SongSummary;
}

function isTypingTarget(el: EventTarget | null): boolean {
  const node = el as HTMLElement | null;
  const tag = node?.tagName;
  return tag === "INPUT" || tag === "TEXTAREA";
}

export function SongLive({ song }: SongLiveProps) {
  const [slides, setSlides] = useState<Slide[]>([]);
  const [index, setIndex] = useState(-1); // -1 = nothing live yet
  // Live editing: which slide is open for correction, and its working text. The
  // screen keeps showing the slide throughout; a typo is not a reason to blank the
  // wall in front of a congregation.
  const [editing, setEditing] = useState<number | null>(null);
  const [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);

  // Refs keep the keydown handler current without re-binding every render.
  const slidesRef = useRef<Slide[]>([]);
  const indexRef = useRef(-1);
  slidesRef.current = slides;
  indexRef.current = index;

  useEffect(() => {
    let active = true;
    getSongSlides(song.id).then((s) => {
      if (active) {
        setSlides(s);
        setIndex(-1);
      }
    });
    return () => {
      active = false;
    };
  }, [song.id]);

  // Open the live slide for correction. The projection is left exactly as it is.
  async function startEdit(i: number): Promise<void> {
    const detail = await getSong(song.id).catch(() => null);
    if (!detail) return;
    setDraft(blockAt(detail.lyrics, i));
    setEditing(i);
  }

  // Save the correction and put the same slide straight back up.
  //
  // Re-projecting the same index rather than blanking is the whole point: a typo
  // spotted mid-song gets fixed and the congregation sees the line change, not the
  // screen go dark and come back.
  async function saveEdit(): Promise<void> {
    if (editing === null) return;
    setSaving(true);
    try {
      const detail = await getSong(song.id);
      if (!detail) return;
      await updateSong(song.id, detail.title, detail.author, replaceBlock(detail.lyrics, editing, draft));
      const fresh = await getSongSlides(song.id);
      setSlides(fresh);
      slidesRef.current = fresh;
      const wasLive = indexRef.current;
      setEditing(null);
      if (wasLive >= 0 && wasLive < fresh.length) {
        projectAt(wasLive);
      }
    } catch {
      /* leave the editor open so the correction is not lost */
    } finally {
      setSaving(false);
    }
  }

  function projectAt(i: number): void {
    const all = slidesRef.current;
    if (i < 0 || i >= all.length) return;
    useLiveStore.getState().setOwner("song");
    rememberSong(song.id, song.title);
    setIndex(i);
    void projectSlide(song.id, i);
    const next = all[i + 1];
    void setStage(
      slideSlot(all[i].text, song.title),
      next ? slideSlot(next.text, song.title) : null,
    ).catch(() => {});
  }

  useEffect(() => {
    function onKey(e: KeyboardEvent): void {
      if (isTypingTarget(e.target)) return;
      if (useLiveStore.getState().owner !== "song") return;
      const cur = indexRef.current;
      switch (e.key) {
        case "ArrowRight":
        case "ArrowDown":
        case "PageDown":
        case " ":
          e.preventDefault();
          projectAt(cur < 0 ? 0 : cur + 1);
          break;
        case "ArrowLeft":
        case "ArrowUp":
        case "PageUp":
          e.preventDefault();
          projectAt(cur <= 0 ? 0 : cur - 1);
          break;
        case "Escape":
        case "b":
        case "B":
          e.preventDefault();
          setIndex(-1);
          void blankProjection();
          void clearStage();
          break;
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [song.id]);

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <span className="font-semibold">{song.title}</span>
        <span className="text-xs text-gray-500">
          arrow keys advance · Esc/B blanks
        </span>
      </div>
      <div className="flex gap-2">
        <button onClick={() => projectAt(index <= 0 ? 0 : index - 1)} className="btn btn-sm">
          ◀ Prev
        </button>
        <button onClick={() => projectAt(index < 0 ? 0 : index + 1)} className="btn btn-sm">
          Next ▶
        </button>
        <button
          onClick={() => {
            setIndex(-1);
            void blankProjection();
            void clearStage();
          }}
          className="btn btn-sm"
        >
          Blank
        </button>
        <button
          className="btn btn-sm"
          disabled={index < 0}
          onClick={() => void startEdit(index)}
        >
          Edit slide
        </button>
      </div>
      {/* Correcting a slide without taking it off the wall. */}
      {editing !== null && (
        <div className="space-y-1.5 rounded border p-2" style={{ borderColor: "var(--border)" }}>
          <textarea
            className="input w-full"
            rows={4}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") setEditing(null);
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) void saveEdit();
            }}
          />
          <div className="flex gap-2">
            <button className="btn btn-sm btn-primary" disabled={saving} onClick={() => void saveEdit()}>
              Save
            </button>
            <button className="btn btn-sm" onClick={() => setEditing(null)}>
              Cancel
            </button>
          </div>
        </div>
      )}

      {/* Straight to a section. The operator follows the worship leader, not the
          printed order: back to the chorus, skip a verse, round again to the bridge.
          Hunting for the right row in the list is slower than the leader is. */}
      {sections(slides).length > 1 && (
        <div className="flex flex-wrap gap-1">
          {sections(slides).map((sec) => (
            <button
              key={sec.label}
              onClick={() => projectAt(sec.orderIndex)}
              className="btn btn-sm"
              aria-label={sec.label}
            >
              {shortLabel(sec.label)}
            </button>
          ))}
        </div>
      )}

      <div className="space-y-1">
        {slides.map((slide) => (
          <button
            key={slide.orderIndex}
            onClick={() => projectAt(slide.orderIndex)}
            className={`block w-full rounded border p-2 text-left ${
              slide.orderIndex === index ? "tint tint-current" : "tint-neutral tint-hover"
            }`}
          >
            <div className="flex items-center gap-1.5 text-[10px] text-[var(--faint)]">
              {slide.label && (
                <span className="rounded bg-[var(--tint-neutral,#8883)] px-1.5 py-0.5 font-medium uppercase tracking-wide">
                  {slide.label}
                </span>
              )}
              <span>Slide {slide.orderIndex + 1}</span>
            </div>
            <p className="whitespace-pre-line text-sm">{slide.text}</p>
          </button>
        ))}
      </div>
    </div>
  );
}
