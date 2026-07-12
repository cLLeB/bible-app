import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getProjection, type ProjectionState } from "./api";

function currentText(s: ProjectionState): { body: string; caption: string } {
  switch (s.kind) {
    case "verse":
    case "song":
      return { body: s.text, caption: s.caption };
    case "message":
      return { body: s.text, caption: "" };
    case "countdown":
      return { body: s.label || "Countdown", caption: "" };
    default:
      return { body: "—", caption: "" };
  }
}

export function StageView() {
  const [state, setState] = useState<ProjectionState>({ kind: "blank" });
  const [now, setNow] = useState(new Date());

  useEffect(() => {
    getProjection().then(setState).catch(() => {});
    const sub = listen<ProjectionState>("set-projection", (e) => setState(e.payload));
    const clock = setInterval(() => setNow(new Date()), 1000);
    return () => {
      sub.then((f) => f());
      clearInterval(clock);
    };
  }, []);

  const { body, caption } = currentText(state);
  const time = now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });

  return (
    <div className="flex h-screen w-screen flex-col bg-neutral-900 p-6 text-white">
      <div className="flex items-baseline justify-between border-b border-neutral-700 pb-2">
        <span className="text-sm uppercase tracking-widest text-neutral-400">Stage Display</span>
        <span className="font-mono text-3xl tabular-nums">{time}</span>
      </div>
      <div className="flex flex-1 flex-col items-center justify-center text-center">
        <p className="mb-6 max-w-4xl whitespace-pre-line text-4xl leading-tight">{body}</p>
        {caption && <p className="text-xl text-neutral-400">{caption}</p>}
      </div>
    </div>
  );
}
