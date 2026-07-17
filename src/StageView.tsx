import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getStage, type StageInfo } from "./api";
import { stageTimerElapsed, stageTimerText } from "./lib/stageTimer";

const EMPTY: StageInfo = { current: null, next: null, message: "", timer: { mode: "off", anchorMs: 0 } };

export function StageView() {
  const [stage, setStage] = useState<StageInfo>(EMPTY);
  const [now, setNow] = useState(new Date());

  useEffect(() => {
    getStage().then(setStage).catch(() => {});
    const sub = listen<StageInfo>("set-stage", (e) => setStage(e.payload));
    const clock = setInterval(() => setNow(new Date()), 1000);
    return () => {
      sub.then((f) => f());
      clearInterval(clock);
    };
  }, []);

  const time = now.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

  const timer = stage.timer ?? EMPTY.timer;
  const timerText = stageTimerText(timer, now.getTime());
  const timerOver = stageTimerElapsed(timer, now.getTime());

  return (
    <div className="flex h-screen w-screen flex-col bg-neutral-950 p-6 text-white">
      {/* Header: label + timer + live clock */}
      <div className="flex items-baseline justify-between border-b border-neutral-800 pb-2">
        <span className="text-sm uppercase tracking-widest text-neutral-500">Stage Display</span>
        {timerText !== null && (
          <span
            className="font-mono text-5xl font-bold tabular-nums"
            style={{ color: timerOver ? "#f87171" : "#4ade80" }}
          >
            {timerText}
          </span>
        )}
        <span className="font-mono text-3xl tabular-nums text-neutral-200">{time}</span>
      </div>

      {/* Private operator message — flashes across the top, never on the wall */}
      {stage.message.trim() !== "" && (
        <div className="mt-3 animate-pulse rounded-lg bg-amber-500 px-5 py-3 text-center text-3xl font-bold text-black">
          {stage.message}
        </div>
      )}

      {/* CURRENT — the big live line */}
      <div className="flex flex-1 flex-col items-center justify-center text-center">
        {stage.current ? (
          <>
            <p className="max-w-5xl whitespace-pre-line text-5xl font-semibold leading-tight">
              {stage.current.text}
            </p>
            {stage.current.caption && (
              <p className="mt-4 text-2xl text-neutral-400">{stage.current.caption}</p>
            )}
          </>
        ) : (
          <p className="text-2xl text-neutral-600">Nothing live</p>
        )}
      </div>

      {/* NEXT — dimmed preview strip, only the platform sees it */}
      <div className="border-t border-neutral-800 pt-3">
        <div className="mb-1 text-xs uppercase tracking-widest text-neutral-600">Next</div>
        {stage.next ? (
          <div className="flex items-baseline gap-3">
            <p className="line-clamp-2 flex-1 whitespace-pre-line text-2xl leading-snug text-neutral-300">
              {stage.next.text}
            </p>
            {stage.next.caption && (
              <span className="shrink-0 text-lg text-neutral-500">{stage.next.caption}</span>
            )}
          </div>
        ) : (
          <p className="text-2xl text-neutral-700">— End —</p>
        )}
      </div>
    </div>
  );
}
