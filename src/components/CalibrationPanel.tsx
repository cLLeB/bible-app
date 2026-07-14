import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  calibrationScript,
  recordCalibrationLine,
  runCalibration,
  type CalibrationResult,
  type ConfigScore,
  type ScriptLine,
  type SttModel,
} from "../api";

interface CalibrationPanelProps {
  model: SttModel;
  disabled: boolean;
}

type Phase = "idle" | "recording" | "scoring" | "done";

/**
 * Read a short script aloud; the app then replays those recordings through every
 * candidate recognizer setting and keeps whichever one finds the most scripture.
 * What suits one voice suits another badly — accent, pace, mic and room all move
 * the answer — so this is measured per speaker rather than guessed once for all.
 */
export function CalibrationPanel({ model, disabled }: CalibrationPanelProps) {
  const [script, setScript] = useState<ScriptLine[]>([]);
  const [phase, setPhase] = useState<Phase>("idle");
  const [current, setCurrent] = useState(0);
  const [recorded, setRecorded] = useState<Set<number>>(new Set());
  const [progress, setProgress] = useState<ConfigScore[]>([]);
  const [result, setResult] = useState<CalibrationResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void calibrationScript().then(setScript).catch(() => setScript([]));
    const un = listen<ConfigScore>("calibration-progress", (e) =>
      setProgress((prev) => [...prev, e.payload]),
    );
    return () => {
      void un.then((f) => f());
    };
  }, []);

  async function recordLine(index: number): Promise<void> {
    setError(null);
    setPhase("recording");
    setCurrent(index);
    try {
      await recordCalibrationLine(index);
      setRecorded((prev) => new Set(prev).add(index));
      // Walk to the next unrecorded line so a full pass is just click, speak, click.
      const next = script.findIndex((l) => l.index > index && !recorded.has(l.index));
      setCurrent(next === -1 ? index : next);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setPhase("idle");
    }
  }

  async function score(): Promise<void> {
    setError(null);
    setProgress([]);
    setPhase("scoring");
    try {
      setResult(await runCalibration(model));
      setPhase("done");
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
      setPhase("idle");
    }
  }

  const done = recorded.size;
  const total = script.length;

  return (
    <details className="rounded-lg border" style={{ borderColor: "var(--border)" }}>
      <summary className="cursor-pointer px-3 py-2 text-sm font-medium">
        Voice calibration{" "}
        <span className="text-[var(--muted)]">
          — teach it your voice ({done}/{total} lines recorded)
        </span>
      </summary>

      <div className="space-y-3 px-3 pb-3">
        <p className="text-sm text-[var(--muted)]">
          Read each line the way you'd actually say it in a service. The app then replays
          your recordings through every recognizer setting and keeps the one that finds the
          most scripture. Nothing leaves this machine.
        </p>

        {error && <p className="text-sm text-red-500">{error}</p>}

        <ol className="space-y-1">
          {script.map((line) => (
            <li
              key={line.index}
              className="flex items-center gap-2 text-sm"
              style={{ opacity: line.index === current || recorded.has(line.index) ? 1 : 0.65 }}
            >
              <span className="w-5 text-right text-[var(--muted)]">{line.index + 1}.</span>
              <span className="flex-1">“{line.say}”</span>
              {recorded.has(line.index) && <span style={{ color: "var(--live)" }}>✓</span>}
              <button
                className="btn"
                disabled={disabled || phase !== "idle"}
                onClick={() => void recordLine(line.index)}
              >
                {phase === "recording" && current === line.index
                  ? "listening…"
                  : recorded.has(line.index)
                    ? "redo"
                    : "record"}
              </button>
            </li>
          ))}
        </ol>

        <div className="flex flex-wrap items-center gap-2">
          <button
            className="btn btn-primary"
            disabled={disabled || done === 0 || phase !== "idle"}
            onClick={() => void score()}
          >
            {phase === "scoring" ? "Comparing settings…" : `Tune ${model} on my voice`}
          </button>
          {disabled && (
            <span className="text-sm text-[var(--muted)]">Stop listening first.</span>
          )}
        </div>

        {phase === "scoring" && progress.length > 0 && (
          <ul className="space-y-1 text-sm text-[var(--muted)]">
            {progress.map((p) => (
              <li key={p.label}>
                {p.label}: {p.resolved}/{p.total} found · {p.secondsPerClip.toFixed(1)}s
              </li>
            ))}
          </ul>
        )}

        {result && phase === "done" && (
          <div className="space-y-1 text-sm">
            <p>
              <strong>Now using: {result.best.label}</strong> — found{" "}
              {result.best.resolved} of {result.best.total} references,{" "}
              {result.best.secondsPerClip.toFixed(1)}s each.
            </p>
            <p className="text-[var(--muted)]">
              The shipped default ({result.baseline.label}) found {result.baseline.resolved} of{" "}
              {result.baseline.total} on your voice.
            </p>
          </div>
        )}
      </div>
    </details>
  );
}
