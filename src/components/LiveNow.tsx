import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { blankProjection, getProjection, type ProjectionState } from "../api";

function describe(s: ProjectionState): string {
  switch (s.kind) {
    case "verse":
    case "song":
      return s.caption || s.text.slice(0, 60);
    case "message":
      return `Message: ${s.text.slice(0, 50)}`;
    case "countdown":
      return `Countdown${s.label ? ` — ${s.label}` : ""}`;
    case "logo":
      return "Logo";
    case "blackout":
      return "Blackout";
    default:
      return "Nothing (blank)";
  }
}

export function LiveNow() {
  const [state, setState] = useState<ProjectionState>({ kind: "blank" });

  useEffect(() => {
    getProjection().then(setState).catch(() => {});
    const sub = listen<ProjectionState>("set-projection", (e) => setState(e.payload));
    return () => {
      sub.then((f) => f());
    };
  }, []);

  const live = state.kind !== "blank" && state.kind !== "blackout";

  return (
    <div className="flex items-center gap-3 rounded border bg-neutral-50 px-3 py-2">
      <span
        className={`inline-block h-2.5 w-2.5 rounded-full ${live ? "bg-red-600" : "bg-gray-300"}`}
      />
      <span className="text-xs uppercase tracking-wide text-gray-500">On screen</span>
      <span className="flex-1 truncate text-sm">{describe(state)}</span>
      <button onClick={() => blankProjection()} className="rounded border px-2 py-0.5 text-xs">
        Blank
      </button>
    </div>
  );
}
