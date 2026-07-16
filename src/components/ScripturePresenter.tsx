import { FormEvent, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { navigate, presentCoords, type NavDir, type VersePayload } from "../api";
import { useLiveStore, useScriptureStore } from "../services";
import { pushScriptureStage } from "../stage";

function isTypingTarget(el: EventTarget | null): boolean {
  const tag = (el as HTMLElement | null)?.tagName;
  return tag === "INPUT" || tag === "TEXTAREA";
}

export function ScripturePresenter() {
  const { current, setCurrent } = useScriptureStore();
  const [jump, setJump] = useState("");
  const curRef = useRef<VersePayload | null>(null);
  curRef.current = current;

  // Mirror the live scripture verse (and its next verse) onto the stage monitor.
  // Only when scripture owns the screen — during a service the ServicePanel
  // drives the stage so "next" reflects the run order, not the next chapter verse.
  useEffect(() => {
    if (current && useLiveStore.getState().owner === "scripture") {
      void pushScriptureStage(current);
    }
  }, [current]);

  // Voice navigation projects from the backend; mirror it here so the
  // presenter's "current" (and keyboard nav) stays in sync with the screen.
  useEffect(() => {
    const sub = listen<{ verse: VersePayload; source: string }>("verse-candidate", (e) => {
      if (e.payload.source === "voice-nav") {
        useLiveStore.getState().setOwner("scripture");
        setCurrent(e.payload.verse);
      }
    });
    return () => {
      sub.then((f) => f());
    };
  }, [setCurrent]);

  async function go(dir: NavDir): Promise<void> {
    useLiveStore.getState().setOwner("scripture");
    const v = await navigate(dir);
    if (v) setCurrent(v);
  }

  useEffect(() => {
    function onKey(e: KeyboardEvent): void {
      if (isTypingTarget(e.target)) return;
      if (useLiveStore.getState().owner !== "scripture" || !curRef.current) return;
      if (["ArrowRight", "ArrowDown", " "].includes(e.key)) {
        e.preventDefault();
        void go("next-verse");
      } else if (["ArrowLeft", "ArrowUp"].includes(e.key)) {
        e.preventDefault();
        void go("prev-verse");
      } else if (e.key === "PageDown") {
        e.preventDefault();
        void go("next-chapter");
      } else if (e.key === "PageUp") {
        e.preventDefault();
        void go("prev-chapter");
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  async function onJump(e: FormEvent): Promise<void> {
    e.preventDefault();
    if (!current) return;
    const t = jump.trim();
    let chapter = current.chapter;
    let verse: number;
    if (t.includes(":")) {
      const [c, v] = t.split(":");
      chapter = Number(c) || current.chapter;
      verse = Number(v) || 1;
    } else {
      verse = Number(t) || 1;
    }
    try {
      const res = await presentCoords(current.bookOsis, chapter, verse);
      useLiveStore.getState().setOwner("scripture");
      setCurrent(res);
      setJump("");
    } catch {
      /* out of range — ignore */
    }
  }

  if (!current) return null;

  return (
    <div className="tint tint-current flex flex-wrap items-center gap-2 rounded-xl border p-2.5">
      <span className="chip" style={{ color: "var(--live)" }}>Presenting</span>
      <span className="font-semibold">{current.reference}</span>
      <span className="hidden flex-1 truncate text-xs text-[var(--muted)] lg:inline">— {current.text}</span>
      <div className="ml-auto flex items-center gap-1">
        <button onClick={() => go("prev-chapter")} className="btn btn-sm" title="Previous chapter (PageUp)">⏮ ch</button>
        <button onClick={() => go("prev-verse")} className="icon-btn" title="Previous verse (←)">◀</button>
        <button onClick={() => go("next-verse")} className="icon-btn" title="Next verse (→)">▶</button>
        <button onClick={() => go("next-chapter")} className="btn btn-sm" title="Next chapter (PageDown)">ch ⏭</button>
        <form onSubmit={onJump}>
          <input
            value={jump}
            onChange={(e) => setJump(e.target.value)}
            placeholder="15 or 4:5"
            className="input h-9 w-20 text-sm"
            title="Jump to verse, or chapter:verse, in this book"
          />
        </form>
      </div>
    </div>
  );
}
