import { useEffect, useState } from "react";
import { audioInputs, setAudioInput, testAudioInput } from "../api";

interface AudioInputPickerProps {
  disabled: boolean;
}

/**
 * Choose where the app listens — and it must be chosen.
 *
 * The app is fed from the sound desk: the preacher's own microphone, already mixed,
 * with no room, no congregation and no PA in it. That is the signal it was built and
 * measured against.
 *
 * There is deliberately no "system default". Defaulting means defaulting to the
 * laptop's own microphone, which would hear the hall instead of the preacher — and
 * would look, from the operator's chair, exactly like it was working. Listening to the
 * wrong thing is worse than not listening at all, so nothing is listened to until an
 * input is picked, and the machine's own microphone is marked as what it is.
 */
export function AudioInputPicker({ disabled }: AudioInputPickerProps) {
  const [devices, setDevices] = useState<string[]>([]);
  const [chosen, setChosen] = useState<string>("");
  const [level, setLevel] = useState<number | null>(null);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function refresh(): Promise<void> {
    try {
      const inputs = await audioInputs();
      setDevices(inputs.all);
      setChosen(inputs.chosen ?? "");
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function choose(name: string): Promise<void> {
    setError(null);
    setLevel(null);
    setChosen(name);
    try {
      await setAudioInput(name === "" ? null : name);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function test(): Promise<void> {
    setError(null);
    setTesting(true);
    setLevel(null);
    try {
      setLevel(await testAudioInput());
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setTesting(false);
    }
  }

  const verdict =
    level === null
      ? null
      : level < 0.01
        ? "nothing arriving — check the cable and that the desk is sending"
        : level < 0.05
          ? "very quiet — turn the send up at the desk"
          : level > 0.98
            ? "clipping — turn the send down"
            : "sound is arriving";

  return (
    <div className="space-y-1">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-sm font-medium">Sound input:</span>
        <select
          className="select"
          style={{ width: "auto", minWidth: "18rem" }}
          value={chosen}
          disabled={disabled}
          onChange={(e) => void choose(e.target.value)}
          title="The sound-desk feed, not the room"
        >
          <option value="">— pick the sound-desk feed —</option>
          {devices.map((d) => (
            <option key={d} value={d}>
              {d}
            </option>
          ))}
        </select>

        <button className="btn" disabled={disabled || testing} onClick={() => void refresh()}>
          Rescan
        </button>
        <button
          className="btn"
          disabled={disabled || testing || chosen === ""}
          onClick={() => void test()}
        >
          {testing ? "listening…" : "Test sound"}
        </button>

        {level !== null && (
          <span className="flex items-center gap-2 text-sm">
            <span
              aria-hidden
              style={{
                display: "inline-block",
                width: "6rem",
                height: "0.5rem",
                borderRadius: "999px",
                background: "var(--surface-2)",
                overflow: "hidden",
              }}
            >
              <span
                style={{
                  display: "block",
                  height: "100%",
                  width: `${Math.min(100, Math.round(level * 100))}%`,
                  background: level < 0.01 ? "var(--danger)" : "var(--live)",
                }}
              />
            </span>
            <span className="text-[var(--muted)]">{verdict}</span>
          </span>
        )}
      </div>

      {chosen === "" && (
        <p className="text-sm text-[var(--muted)]">
          {devices.length === 0
            ? "No input yet — connect the sound-desk feed (USB from the mixer, or a USB audio interface) and press Rescan."
            : "Pick the sound-desk feed to start listening."}
        </p>
      )}

      {error && <p className="text-sm text-red-500">{error}</p>}
    </div>
  );
}
