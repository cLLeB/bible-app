import { useEffect, useRef, useState } from "react";
import {
  blankProjection,
  getSongSlides,
  projectSlide,
  type Slide,
  type SongSummary,
} from "../api";

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

  function projectAt(i: number): void {
    if (i < 0 || i >= slidesRef.current.length) return;
    setIndex(i);
    void projectSlide(song.id, i);
  }

  useEffect(() => {
    function onKey(e: KeyboardEvent): void {
      if (isTypingTarget(e.target)) return;
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
          — arrow keys advance · Esc/B blanks
        </span>
      </div>
      <div className="flex gap-2">
        <button onClick={() => projectAt(index <= 0 ? 0 : index - 1)} className="rounded border px-3 py-1">
          ◀ Prev
        </button>
        <button onClick={() => projectAt(index < 0 ? 0 : index + 1)} className="rounded border px-3 py-1">
          Next ▶
        </button>
        <button
          onClick={() => {
            setIndex(-1);
            void blankProjection();
          }}
          className="rounded border px-3 py-1"
        >
          Blank
        </button>
      </div>
      <div className="space-y-1">
        {slides.map((slide) => (
          <button
            key={slide.orderIndex}
            onClick={() => projectAt(slide.orderIndex)}
            className={`block w-full rounded border p-2 text-left ${
              slide.orderIndex === index ? "border-green-600 bg-green-50" : "hover:bg-gray-50"
            }`}
          >
            <div className="text-[10px] text-gray-400">Slide {slide.orderIndex + 1}</div>
            <p className="whitespace-pre-line text-sm">{slide.text}</p>
          </button>
        ))}
      </div>
    </div>
  );
}
