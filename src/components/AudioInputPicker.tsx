import { useEffect, useState } from "react";
import { audioInputs, setAudioInput, testAudioInput } from "../api";

interface AudioInputPickerProps {
  disabled: boolean;
  /** Fired after a selection is saved, so the panel around it can re-read the state
   *  it shows outside this fold, notably the "room mic" badge. */
  onChanged?: () => void;
}

/**
 * Choose where the app listens, and it must be chosen.
 *
 * For a service the app is fed from the sound desk: the preacher's own microphone,
 * already mixed, with no room, no congregation and no PA in it. That is the signal it
 * was built and measured against, and there is deliberately no "system default",
 * because defaulting means defaulting to the room.
 *
 * This laptop's own microphone is offered too, in its own group, because it is the
 * quickest way to show someone that the app really does hear speech and find the verse:
 * no cable, no mixer, just talk to it. What must never happen is arriving there by
 * accident, because listening to the wrong thing looks, from the operator's chair,
 * exactly like listening to the right one. So it takes a second, deliberate press to
 * select, and it is never a default.
 */
export function AudioInputPicker({ disabled, onChanged }: AudioInputPickerProps) {
  const [devices, setDevices] = useState<string[]>([]);
  const [roomMics, setRoomMics] = useState<string[]>([]);
  const [chosen, setChosen] = useState<string>("");
  /** A room mic picked from the list but not yet confirmed. Nothing is saved yet. */
  const [pendingRoomMic, setPendingRoomMic] = useState<string | null>(null);
  const [level, setLevel] = useState<number | null>(null);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function refresh(): Promise<void> {
    try {
      const inputs = await audioInputs();
      setDevices(inputs.all);
      setRoomMics(inputs.roomMics);
      setChosen(inputs.chosen ?? "");
      setPendingRoomMic(null);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  /** Save a selection. `roomMic` is the operator's explicit yes, never inferred. */
  async function save(name: string, roomMic: boolean): Promise<void> {
    setError(null);
    setLevel(null);
    setChosen(name);
    setPendingRoomMic(null);
    try {
      await setAudioInput(name === "" ? null : name, roomMic);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      // Either way, the badge outside this fold must reflect what was actually
      // stored, not what we hoped to store.
      onChanged?.();
    }
  }

  function choose(name: string): void {
    // Picking the room mic arms a confirmation instead of taking effect. The desk
    // feed stays selected until the operator says, in as many words, that they mean it.
    if (roomMics.includes(name)) {
      setError(null);
      setLevel(null);
      setPendingRoomMic(name);
      return;
    }
    void save(name, false);
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
        ? "nothing arriving: check the cable and that the desk is sending"
        : level < 0.05
          ? "very quiet: turn the send up at the desk"
          : level > 0.98
            ? "clipping: turn the send down"
            : "sound is arriving";

  // While a room mic is pending, show it in the box so the operator can see what they
  // are being asked about, but `chosen` is untouched until they confirm.
  const shown = pendingRoomMic ?? chosen;

  return (
    <div className="space-y-1">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-sm font-medium">Sound input:</span>
        <select
          className="select"
          style={{ width: "auto", minWidth: "18rem" }}
          value={shown}
          disabled={disabled}
          onChange={(e) => choose(e.target.value)}
          title="The sound-desk feed for a service; this laptop's own microphone only to demonstrate"
        >
          <option value="">Pick the sound-desk feed…</option>
          {devices.length > 0 && (
            <optgroup label="From the sound desk">
              {devices.map((d) => (
                <option key={d} value={d}>
                  {d}
                </option>
              ))}
            </optgroup>
          )}
          {roomMics.length > 0 && (
            <optgroup label="Demonstration only (hears the room, not the preacher)">
              {roomMics.map((d) => (
                <option key={d} value={d}>
                  {d}
                </option>
              ))}
            </optgroup>
          )}
        </select>

        <button className="btn" disabled={disabled || testing} onClick={() => void refresh()}>
          Rescan
        </button>
        <button
          className="btn"
          disabled={disabled || testing || chosen === "" || pendingRoomMic !== null}
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

      {/* The deliberate second step. Nothing has been saved at this point. */}
      {pendingRoomMic !== null && (
        <div className="tint tint-warn flex flex-wrap items-center gap-2 rounded border p-2 text-sm">
          <span>
            <strong>{pendingRoomMic}</strong> is the room mic.
          </span>
          <span className="flex gap-2">
            <button className="btn" onClick={() => void save(pendingRoomMic, true)}>
              Use it to demonstrate
            </button>
            <button className="btn" onClick={() => setPendingRoomMic(null)}>
              Cancel
            </button>
          </span>
        </div>
      )}

      {/* The badge on the Live listening header already says the app is on the
          room mic, and the confirmation above explains what that means. Repeating
          it in full here told the operator nothing they had not just read. */}

      {chosen === "" && pendingRoomMic === null && (
        <p className="text-sm text-[var(--muted)]">
          {devices.length === 0
            ? "No sound-desk feed connected. Plug the laptop into the mixer (USB from the desk, or a USB audio interface) and press Rescan. To demonstrate the app without any of that, pick this laptop's own microphone below."
            : "Pick the sound-desk feed to start listening."}
        </p>
      )}

      {error && <p className="text-sm text-red-500">{error}</p>}
    </div>
  );
}
