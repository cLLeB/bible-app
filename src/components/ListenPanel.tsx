import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  appFlavor,
  blankProjection,
  recordChoice,
  startListening,
  stopListening,
  type Candidate,
  type SttModel,
  type VersePayload,
} from "../api";
import { present } from "../present";

const MODEL_LABELS: Record<SttModel, string> = {
  tiny: "Tiny (low-end PCs)",
  base: "Base (normal)",
  small: "Small (accurate, stronger PC)",
  medium: "Medium (best · GPU recommended)",
};

export function ListenPanel() {
  const [listening, setListening] = useState(false);
  const [model, setModel] = useState<SttModel>(
    () => (localStorage.getItem("stt-model") as SttModel) || "base",
  );
  const [availableModels, setAvailableModels] = useState<SttModel[]>(["base", "small", "medium"]);
  const [lines, setLines] = useState<string[]>([]);
  const [candidates, setCandidates] = useState<Candidate[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [confirmed, setConfirmed] = useState<string | null>(null);
  const [alternatives, setAlternatives] = useState<VersePayload[]>([]);
  const [autoProject, setAutoProject] = useState(() => localStorage.getItem("auto-project") === "1");
  const [threshold, setThreshold] = useState(() => {
    const s = Number(localStorage.getItem("auto-threshold"));
    return s >= 50 && s <= 100 ? s / 100 : 0.82;
  });

  function changeModel(m: SttModel): void {
    setModel(m);
    localStorage.setItem("stt-model", m);
  }
  function changeAuto(v: boolean): void {
    setAutoProject(v);
    localStorage.setItem("auto-project", v ? "1" : "0");
  }
  function changeThreshold(pct: number): void {
    const v = Math.min(100, Math.max(50, pct || 82));
    setThreshold(v / 100);
    localStorage.setItem("auto-threshold", String(v));
  }

  // This build's flavor decides which whisper models are available.
  useEffect(() => {
    void appFlavor().then((f) => {
      setAvailableModels(f.models);
      setModel((m) => (f.models.includes(m) ? m : f.defaultModel));
    });
  }, []);

  // Refs so the once-registered event listener sees current values.
  const autoRef = useRef(false);
  autoRef.current = autoProject;
  const thresholdRef = useRef(0.82);
  thresholdRef.current = threshold;

  useEffect(() => {
    const unlisteners = [
      listen<string>("transcript", (e) => {
        setLines((prev) => [e.payload, ...prev].slice(0, 6));
      }),
      listen<Candidate>("verse-candidate", (e) => {
        setCandidates((prev) => [e.payload, ...prev].slice(0, 12));
        if (autoRef.current && e.payload.confidence >= thresholdRef.current && e.payload.source !== "voice-nav") {
          void present(e.payload.verse);
        }
      }),
      // The speaker confirmed a suggested verse (said "yes"/"amen" or read it
      // aloud) — project it and mark it confirmed.
      listen<VersePayload>("verse-confirmed", (e) => {
        setConfirmed(e.payload.reference);
        void present(e.payload);
      }),
      // Standby alternatives for the current best guess — operator can pick one.
      listen<VersePayload[]>("verse-alternatives", (e) => {
        setAlternatives(e.payload);
      }),
      listen("listen-started", () => {
        setListening(true);
        setError(null);
      }),
      listen("listen-stopped", () => setListening(false)),
      listen("listen-idle-stop", () => {
        setListening(false);
        setError("Stopped listening after 20 minutes of silence — click to start again.");
      }),
      listen<string>("listen-error", (e) => setError(e.payload)),
    ];
    return () => {
      unlisteners.forEach((u) => u.then((f) => f()));
    };
  }, []);

  // Present a verse and teach the ranker which one the operator chose for the
  // current spoken description.
  function pick(v: VersePayload): void {
    void present(v);
    if (lines[0]) {
      void recordChoice(lines[0], v.bookOsis, v.chapter, v.verse);
    }
  }

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
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <h2 className="panel-title">Live listening</h2>
        {listening && (
          <span className="chip" style={{ color: "var(--live)" }}>
            ● listening…
          </span>
        )}
        <label
          className="ml-auto flex items-center gap-1.5 text-sm text-[var(--muted)]"
          title="Automatically project detections at or above this confidence"
        >
          <input type="checkbox" checked={autoProject} onChange={(e) => changeAuto(e.target.checked)} />
          Auto-project ≥
          <input
            type="number"
            min={50}
            max={100}
            value={Math.round(threshold * 100)}
            onChange={(e) => changeThreshold(Number(e.target.value))}
            disabled={!autoProject}
            className="input w-16 px-2 text-center"
          />
          %
        </label>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <button
          onClick={toggle}
          className={`btn btn-lg ${listening ? "" : "btn-primary"}`}
          style={listening ? { background: "var(--danger)", borderColor: "transparent", color: "#fff" } : undefined}
        >
          {listening ? "■ Stop listening" : "● Start listening"}
        </button>
        <select
          value={model}
          onChange={(e) => changeModel(e.target.value as SttModel)}
          disabled={listening}
          className="select"
          style={{ width: "auto", minWidth: "12rem" }}
          title="Tiny = fastest/low-end · Base = normal · Small = accurate, stronger PC · Medium = most accurate, GPU (CUDA) recommended"
        >
          {availableModels.map((m) => (
            <option key={m} value={m}>
              {MODEL_LABELS[m]}
            </option>
          ))}
        </select>
      </div>

      {error && <p className="rounded-lg bg-red-50 p-2 text-sm text-red-700">{error}</p>}

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div>
          <div className="panel-title mb-1.5">Transcript</div>
          <div className="min-h-24 space-y-1 rounded-lg border p-2.5 text-sm" style={{ background: "var(--surface-2)" }}>
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
          <div className="mb-1.5 flex items-center justify-between">
            <span className="panel-title">Detected verses</span>
            <button onClick={() => blankProjection()} className="btn btn-sm">
              Blank
            </button>
          </div>
          {confirmed && (
            <p className="mb-1 rounded bg-green-50 px-2 py-1 text-xs text-green-700">
              ✓ Confirmed by speaker: {confirmed}
            </p>
          )}
          {alternatives.length > 0 && (
            <div className="mb-1 rounded border border-dashed border-gray-300 p-1.5">
              <div className="mb-1 text-[10px] uppercase text-gray-400">Or did you mean…</div>
              <div className="flex flex-wrap gap-1">
                {alternatives.map((alt, i) => (
                  <button
                    key={`${alt.reference}-${i}`}
                    onClick={() => pick(alt)}
                    className="rounded bg-gray-100 px-2 py-0.5 text-xs hover:bg-green-100"
                    title={alt.text}
                  >
                    {alt.reference}
                  </button>
                ))}
              </div>
            </div>
          )}
          <div className="min-h-24 space-y-1">
            {candidates.length === 0 ? (
              <span className="text-sm text-gray-400">Detected references appear here.</span>
            ) : (
              candidates.map((c, i) => (
                <button
                  key={`${c.verse.reference}-${i}`}
                  onClick={() => pick(c.verse)}
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
