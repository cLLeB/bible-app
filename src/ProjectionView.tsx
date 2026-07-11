import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getProjection, type VersePayload } from "./api";

export function ProjectionView() {
  const [verse, setVerse] = useState<VersePayload | null>(null);

  useEffect(() => {
    // Pull the current state on mount (covers the case where this window
    // opened after project_verse already emitted its event).
    getProjection().then(setVerse).catch(() => setVerse(null));

    const un = listen<VersePayload | null>("set-projection", (e) => setVerse(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);

  return (
    <div className="flex h-screen w-screen flex-col items-center justify-center bg-black px-16 text-center text-white">
      {verse ? (
        <>
          <p className="mb-8 max-w-5xl text-5xl leading-tight">{verse.text}</p>
          <p className="text-2xl text-gray-300">
            {verse.reference} · {verse.translation}
          </p>
        </>
      ) : (
        <p className="text-sm text-gray-700">Projection ready</p>
      )}
    </div>
  );
}
