import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { VersePayload } from "./api";

export function ProjectionView() {
  const [verse, setVerse] = useState<VersePayload | null>(null);

  useEffect(() => {
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
      ) : null}
    </div>
  );
}
