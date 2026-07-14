import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  calibrationScript,
  recordCalibrationLine,
  runCalibration,
  setVoiceProfile,
  voiceProfiles,
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
  const [profiles, setProfiles] = useState<string[]>([]);
  const [who, setWho] = useState<string>("");
  const [phase, setPhase] = useState<Phase>("idle");
  const [current, setCurrent] = useState(0);
  const [recorded, setRecorded] = useState<Set<number>>(new Set());
  const [progress, setProgress] = useState<ConfigScore[]>([]);
  const [result, setResult] = useState<CalibrationResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void calibrationScript().then(setScript).catch(() => setScript([]));
    void voiceProfiles()
      .then((p) => {
        setProfiles(p.all);
        setWho(p.active);
      })
      .catch(() => undefined);
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

  async function changeWho(name: string): Promise<void> {
    if (!name) return;
    try {
      await setVoiceProfile(name);
      setWho(name);
      if (!profiles.includes(name)) setProfiles([...profiles, name]);
      // Another speaker means other recordings and another result.
      setRecorded(new Set());
      setResult(null);
      setProgress([]);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function addWho(): Promise<void> {
    const name = window.prompt("Name of the speaker (e.g. “Guest — Pastor Mensah”)")?.trim();
    if (name) await changeWho(name);
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
          — {who || "speaker"} ({done}/{total} lines recorded)
        </span>
      </summary>

      <div className="space-y-3 px-3 pb-3">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm font-medium">Preaching today:</span>
          <select
            className="select"
            style={{ width: "auto", minWidth: "12rem" }}
            value={who}
            disabled={disabled || phase !== "idle"}
            onChange={(e) => void changeWho(e.target.value)}
          >
            {profiles.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>
          <button className="btn" disabled={disabled || phase !== "idle"} onClick={() => void addWho()}>
            + Add speaker
          </button>
        </div>

        <p className="text-sm text-[var(--muted)]">
          Each speaker is tuned separately, so calibrating a guest never disturbs anyone
          else's settings. Have them read the lines the way they'd actually preach —
          through the same microphone or desk feed the service will use, since that is the
          sound the app has to work with. Nothing leaves this machine.
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
            {phase === "scoring" ? "Comparing settings…" : `Tune ${model} for ${who || "this speaker"}`}
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
              {result.baseline.total} on this voice. Only {who}'s settings changed.
            </p>
          </div>
        )}
      </div>
    </details>
  );
}
