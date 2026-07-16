import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AudioInputPicker } from "./AudioInputPicker";
import { CalibrationPanel } from "./CalibrationPanel";
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
import { useServiceStore, type Cue } from "../services";

// A medium-confidence hearing that lands on a chapter already on the run sheet is
// certain enough to project on its own: the expectation resolves the doubt. Below this
// floor the hearing itself was too weak to trust, even with that corroboration — so we
// still only suggest it and let the operator decide.
const RUN_SHEET_ASSIST_FLOOR = 0.72;

// Is this verse's chapter one the operator has queued on the run sheet? Book + chapter,
// not the exact verse: the preacher going to that chapter is the corroboration that
// matters, and she may read a different verse within it than the one queued.
function onRunSheet(cues: Cue[], v: { bookOsis: string; chapter: number }): boolean {
  return cues.some(
    (c) => c.type === "verse" && c.verse.bookOsis === v.bookOsis && c.verse.chapter === v.chapter,
  );
}

export function ListenPanel() {
  const [listening, setListening] = useState(false);
  // Fixed by this build's flavor (base-personal / small-personal), not operator-switchable.
  const [model, setModel] = useState<SttModel>("base");
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
  const [useRunSheet, setUseRunSheet] = useState(() => localStorage.getItem("use-run-sheet") === "1");
  const cues = useServiceStore((s) => s.cues);
  const runSheetVerses = cues.filter((c) => c.type === "verse").length;

  function changeAuto(v: boolean): void {
    setAutoProject(v);
    localStorage.setItem("auto-project", v ? "1" : "0");
  }
  function changeRunSheet(v: boolean): void {
    setUseRunSheet(v);
    localStorage.setItem("use-run-sheet", v ? "1" : "0");
  }
  function changeThreshold(pct: number): void {
    const v = Math.min(100, Math.max(50, pct || 82));
    setThreshold(v / 100);
    localStorage.setItem("auto-threshold", String(v));
  }

  // This build's flavor decides the model; the operator doesn't choose it.
  useEffect(() => {
    void appFlavor().then((f) => setModel(f.defaultModel));
  }, []);

  // Refs so the once-registered event listener sees current values.
  const autoRef = useRef(false);
  autoRef.current = autoProject;
  const thresholdRef = useRef(0.82);
  thresholdRef.current = threshold;
  const runSheetRef = useRef(false);
  runSheetRef.current = useRunSheet;

  useEffect(() => {
    const unlisteners = [
      listen<string>("transcript", (e) => {
        setLines((prev) => [e.payload, ...prev].slice(0, 6));
      }),
      listen<Candidate>("verse-candidate", (e) => {
        const c = e.payload;
        setCandidates((prev) => [c, ...prev].slice(0, 12));
        if (!autoRef.current || c.source === "voice-nav") return;
        // Confident enough on its own → project.
        if (c.confidence >= thresholdRef.current) {
          void present(c.verse);
          return;
        }
        // Not sure enough on the hearing alone — but if it lands on a chapter the
        // operator queued on the run sheet, that expectation makes it certain. Only a
        // decent hearing qualifies (>= floor), and only a match to the run sheet: a
        // detection off the run sheet is never forced, and a poor hearing stays a mere
        // suggestion. Better to leave it to the operator than to project a wrong verse.
        if (
          runSheetRef.current &&
          c.confidence >= RUN_SHEET_ASSIST_FLOOR &&
          onRunSheet(useServiceStore.getState().cues, c.verse)
        ) {
          void present(c.verse);
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
          title="Auto-project a medium match when its chapter is on the run sheet"
        >
          <input
            type="checkbox"
            checked={useRunSheet}
            disabled={!autoProject}
            onChange={(e) => changeRunSheet(e.target.checked)}
          />
          Use run sheet{runSheetVerses > 0 ? ` (${runSheetVerses})` : ""}
        </label>
        <label
          className="flex items-center gap-1.5 text-sm text-[var(--muted)]"
          title="Auto-project at or above this confidence"
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
      </div>

      {error && <p className="rounded-lg bg-red-50 p-2 text-sm text-red-700">{error}</p>}

      <AudioInputPicker disabled={listening} />

      <CalibrationPanel model={model} disabled={listening} />

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
                    <span className="flex items-center gap-1">
                      {onRunSheet(cues, c.verse) && (
                        <span
                          className="rounded bg-blue-100 px-1.5 py-0.5 text-[10px] text-blue-700"
                          title="This chapter is on the run sheet"
                        >
                          on run sheet
                        </span>
                      )}
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
