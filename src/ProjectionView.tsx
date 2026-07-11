import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getProjection, type ProjectionState } from "./api";

export function ProjectionView() {
  const [state, setState] = useState<ProjectionState>({ kind: "blank" });

  useEffect(() => {
    // Pull the current state on mount (covers the case where this window
    // opened after a project command already emitted its event).
    getProjection().then(setState).catch(() => setState({ kind: "blank" }));

    const un = listen<ProjectionState>("set-projection", (e) => setState(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);

  return (
    <div className="flex h-screen w-screen flex-col items-center justify-center bg-black px-16 text-center text-white">
      {state.kind === "blank" ? (
        <p className="text-sm text-gray-700">Projection ready</p>
      ) : (
        <>
          <p className="mb-8 max-w-5xl whitespace-pre-line text-5xl leading-tight">{state.text}</p>
          <p className="text-2xl text-gray-300">{state.caption}</p>
        </>
      )}
    </div>
  );
}
