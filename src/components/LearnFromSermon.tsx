import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  learnFromRecordings,
  PROTECTED_PROFILES,
  type LearnProgress,
  type LearnResult,
  type SttModel,
} from "../api";

interface LearnFromSermonProps {
  model: SttModel;
  who: string;
  disabled: boolean;
}

/**
 * Teach the app a preacher from recordings of them preaching.
 *
 * Reading twelve lines at a laptop tunes the app for someone reading a script. Sermons
 * tune it for a preacher: their pace, their accent, the way they name a reference, the
 * version they read from, and how loud their sound desk actually runs.
 *
 * Nobody marks anything up. Whatever the preacher reads aloud is a verse that was right
 * that day, so the recordings carry their own answer key.
 */
export function LearnFromSermon({ model, who, disabled }: LearnFromSermonProps) {
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<LearnProgress | null>(null);
  const [result, setResult] = useState<LearnResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const un = listen<LearnProgress>("learn-progress", (e) => setProgress(e.payload));
    return () => {
      void un.then((f) => f());
    };
  }, []);

  async function run(): Promise<void> {
    setError(null);
    setResult(null);
    const picked = await open({
      multiple: true,
      title: `Recordings of ${who || "this speaker"} preaching`,
      filters: [
        { name: "Audio", extensions: ["mp3", "m4a", "wav", "flac", "aac", "mp4", "ogg", "wma"] },
      ],
    });
    const paths = Array.isArray(picked) ? picked : typeof picked === "string" ? [picked] : [];
    if (paths.length === 0) return;

    // Learning rebuilds the profile from only these files — for a baked-in preacher that
    // means overwriting the shipped tuning. Warn before that; a new profile keeps it.
    if (
      PROTECTED_PROFILES.includes(who) &&
      !window.confirm(`Replace ${who}'s profile? Add a new one to keep it.`)
    ) {
      return;
    }

    setRunning(true);
    setProgress(null);
    try {
      setResult(await learnFromRecordings(paths, model));
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setRunning(false);
      setProgress(null);
    }
  }

  function timeLeft(seconds: number): string {
    if (seconds < 90) return `${Math.max(1, Math.round(seconds))}s left`;
    const mins = Math.round(seconds / 60);
    return mins < 60 ? `${mins} min left` : `${Math.floor(mins / 60)}h ${mins % 60}m left`;
  }

  return (
    <div className="space-y-2 rounded-lg border p-3" style={{ borderColor: "var(--border)" }}>
      <div className="flex flex-wrap items-center gap-2">
        <button className="btn btn-primary" disabled={disabled || running} onClick={() => void run()}>
          {running ? "Learning…" : "Learn from sermon recordings"}
        </button>
        <span className="text-sm text-[var(--muted)]">
          teaches {who || "this speaker"} from how they actually preach
        </span>
      </div>


      {running && progress && (
        <div className="space-y-1">
          <div
            aria-hidden
            style={{
              height: "0.5rem",
              borderRadius: "999px",
              background: "var(--surface-2)",
              overflow: "hidden",
            }}
          >
            <div
              style={{
                height: "100%",
                width: `${progress.percent}%`,
                background: "var(--live)",
                transition: "width .3s",
              }}
            />
          </div>
          <p className="text-sm text-[var(--muted)]">
            {progress.percent}% · {progress.stage}
            {progress.total > 0 ? ` (${progress.done}/${progress.total})` : ""}
            {progress.secondsLeft > 0 ? ` · ${timeLeft(progress.secondsLeft)}` : ""}
          </p>
        </div>
      )}

      {error && <p className="text-sm text-red-500">{error}</p>}

      {result && (
        <div className="space-y-1 text-sm">
          <p>
            <strong>{result.profile} is tuned.</strong> From {result.minutes.toFixed(0)} minutes
            across {result.recordings} recording{result.recordings === 1 ? "" : "s"}, it found{" "}
            {result.referencesFound} passages read aloud and now recovers{" "}
            <strong>
              {result.after} of {result.referencesFound}
            </strong>{" "}
            (the shipped settings found {result.before}).
          </p>
          <ul className="ml-4 list-disc space-y-0.5 text-[var(--muted)]">
            <li>Recognizer: {result.settings}</li>
            {result.translation && <li>Reads from: {result.translation}, now selected for them</li>}
            <li>Speech threshold for their sound feed: {result.speechAbove.toFixed(3)}</li>
            {result.learnedNames.length > 0 && (
              <li>
                Book names heard as: {result.learnedNames.map((n) => `“${n}”`).join(", ")}
              </li>
            )}
          </ul>
        </div>
      )}
    </div>
  );
}
