import { useEffect, useState } from "react";
import { audioInputs, setAudioInput, testAudioInput } from "../api";

interface AudioInputPickerProps {
  disabled: boolean;
}

const DEFAULT = "__default__";

/**
 * Choose where the app listens.
 *
 * The laptop microphone hears the room — reverb, the congregation, whatever the PA
 * throws back. A feed from the sound desk carries the preacher's own microphone,
 * already mixed, with none of that in it. It is the same kind of signal the
 * recognizer was measured against, so plugging into the desk is worth more than any
 * setting in this app.
 *
 * The test button exists because the worst way to discover the cable is dead is
 * halfway through a sermon.
 */
export function AudioInputPicker({ disabled }: AudioInputPickerProps) {
  const [devices, setDevices] = useState<string[]>([]);
  const [chosen, setChosen] = useState<string>(DEFAULT);
  const [level, setLevel] = useState<number | null>(null);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function refresh(): Promise<void> {
    try {
      const inputs = await audioInputs();
      setDevices(inputs.all);
      setChosen(inputs.chosen ?? DEFAULT);
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
      await setAudioInput(name === DEFAULT ? null : name);
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

  // Anything above a whisper registers here; silence means nothing is arriving.
  const verdict =
    level === null
      ? null
      : level < 0.01
        ? "nothing arriving — check the cable and that the desk is sending"
        : level < 0.05
          ? "very quiet — turn the send up if you can"
          : level > 0.98
            ? "clipping — turn the send down"
            : "sound is arriving";

  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-sm font-medium">Sound input:</span>
      <select
        className="select"
        style={{ width: "auto", minWidth: "16rem" }}
        value={chosen}
        disabled={disabled}
        onChange={(e) => void choose(e.target.value)}
        title="Pick the sound desk feed if you have one — it hears the preacher's microphone, not the room."
      >
        <option value={DEFAULT}>System default (laptop microphone)</option>
        {devices.map((d) => (
          <option key={d} value={d}>
            {d}
          </option>
        ))}
      </select>

      <button className="btn" disabled={disabled || testing} onClick={() => void refresh()}>
        Rescan
      </button>
      <button className="btn" disabled={disabled || testing} onClick={() => void test()}>
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

      {error && <span className="text-sm text-red-500">{error}</span>}
    </div>
  );
}
