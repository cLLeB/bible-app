import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { blankProjection, startListening, stopListening, type Candidate } from "../api";
import { present } from "../present";

export function ListenPanel() {
  const [listening, setListening] = useState(false);
  const [model, setModel] = useState<"base" | "tiny">(
    () => (localStorage.getItem("stt-model") as "base" | "tiny") || "base",
  );
  const [lines, setLines] = useState<string[]>([]);
  const [candidates, setCandidates] = useState<Candidate[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [autoProject, setAutoProject] = useState(() => localStorage.getItem("auto-project") === "1");

  function changeModel(m: "base" | "tiny"): void {
    setModel(m);
    localStorage.setItem("stt-model", m);
  }
  function changeAuto(v: boolean): void {
    setAutoProject(v);
    localStorage.setItem("auto-project", v ? "1" : "0");
  }

  // Ref so the once-registered event listener sees the current toggle value.
  const autoRef = useRef(false);
  autoRef.current = autoProject;

  useEffect(() => {
    const unlisteners = [
      listen<string>("transcript", (e) => {
        setLines((prev) => [e.payload, ...prev].slice(0, 6));
      }),
      listen<Candidate>("verse-candidate", (e) => {
        setCandidates((prev) => [e.payload, ...prev].slice(0, 12));
        if (autoRef.current && e.payload.confidence >= 0.9 && e.payload.source !== "voice-nav") {
          void present(e.payload.verse);
        }
      }),
      listen("listen-started", () => {
        setListening(true);
        setError(null);
      }),
      listen("listen-stopped", () => setListening(false)),
      listen<string>("listen-error", (e) => setError(e.payload)),
    ];
    return () => {
      unlisteners.forEach((u) => u.then((f) => f()));
    };
  }, []);

  async function toggle(): Promise<void> {
    try {
      if (listening) {
        await stopListening();
        setListening(false);
      } else {
        setError(null);
        await startListening(model);
        setListening(true);
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <section className="space-y-3">
      <div className="flex items-center gap-3">
        <h2 className="text-xl font-semibold">Live listening</h2>
        <button
          onClick={toggle}
          className={`rounded px-4 py-2 text-white ${listening ? "bg-red-600" : "bg-green-600"}`}
        >
          {listening ? "■ Stop" : "● Start listening"}
        </button>
        <select
          value={model}
          onChange={(e) => changeModel(e.target.value as "base" | "tiny")}
          disabled={listening}
          className="rounded border px-2 py-2 text-sm"
          title="Base = normal accuracy; Tiny = faster on low-end PCs"
        >
          <option value="base">Base (normal)</option>
          <option value="tiny">Tiny (low-end PCs)</option>
        </select>
        {listening && <span className="text-sm text-green-700">listening…</span>}
        <label className="ml-auto flex items-center gap-1 text-sm" title="Automatically project detections at 90%+ confidence">
          <input
            type="checkbox"
            checked={autoProject}
            onChange={(e) => changeAuto(e.target.checked)}
          />
          Auto-project ≥90%
        </label>
      </div>

      {error && <p className="rounded bg-red-50 p-2 text-red-700">{error}</p>}

      <div className="grid grid-cols-2 gap-4">
        <div>
          <div className="mb-1 text-xs uppercase text-gray-500">Transcript</div>
          <div className="min-h-24 space-y-1 rounded border p-2 text-sm">
            {lines.length === 0 ? (
              <span className="text-gray-400">Speak a reference, e.g. “John chapter 3 verse 16”.</span>
            ) : (
              lines.map((l, i) => (
                <p key={i} className={i === 0 ? "text-black" : "text-gray-400"}>
                  {l}
                </p>
              ))
            )}
          </div>
        </div>

        <div>
          <div className="mb-1 flex items-center justify-between text-xs uppercase text-gray-500">
            <span>Detected verses</span>
            <button onClick={() => blankProjection()} className="rounded border px-2 py-0.5 normal-case">
              Blank
            </button>
          </div>
          <div className="min-h-24 space-y-1">
            {candidates.length === 0 ? (
              <span className="text-sm text-gray-400">Detected references appear here.</span>
            ) : (
              candidates.map((c, i) => (
                <button
                  key={`${c.verse.reference}-${i}`}
                  onClick={() => present(c.verse)}
                  className="block w-full rounded border p-2 text-left hover:bg-green-50"
                >
                  <div className="flex items-center justify-between">
                    <span className="text-sm font-semibold">{c.verse.reference}</span>
                    <span
                      className={`rounded px-1.5 py-0.5 text-[10px] ${
                        c.confidence >= 0.9
                          ? "bg-green-100 text-green-700"
                          : c.confidence >= 0.8
                            ? "bg-yellow-100 text-yellow-700"
                            : "bg-gray-100 text-gray-600"
                      }`}
                      title={`${c.source} match`}
                    >
                      {Math.round(c.confidence * 100)}% · {c.source}
                    </span>
                  </div>
                  <div className="line-clamp-2 text-xs text-gray-600">{c.verse.text}</div>
                </button>
              ))
            )}
          </div>
        </div>
      </div>
    </section>
  );
}
