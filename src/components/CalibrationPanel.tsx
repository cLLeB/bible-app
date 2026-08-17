import { useEffect, useState } from "react";
import { LearnFromSermon } from "./LearnFromSermon";
import {
  PROTECTED_PROFILES,
  forgetRecordings,
  learningStatus,
  recordingConsent,
  removeVoiceProfile,
  resetProfileToBaked,
  setRecordingConsent,
  setVoiceProfile,
  voiceProfiles,
  type SttModel,
} from "../api";

interface CalibrationPanelProps {
  model: SttModel;
  disabled: boolean;
  /** Told whenever the speaker, or what they have agreed to, changes. */
  onProfileChange?: () => void;
}

/**
 * Pick who is preaching, and teach the app new speakers from their sermons. Settings
 * are stored per speaker, so each is tuned on their own. The President and
 * Vice-President are baked in and can't be removed.
 */
export function CalibrationPanel({ model, disabled, onProfileChange }: CalibrationPanelProps) {
  const [profiles, setProfiles] = useState<string[]>([]);
  const [who, setWho] = useState<string>("");
  const [consent, setConsent] = useState(false);
  // Can this speaker be put back exactly as the app shipped them? True only for the
  // preachers baked into this build.
  const [canReset, setCanReset] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function load(): void {
    void voiceProfiles()
      .then((p) => {
        setProfiles(p.all);
        setWho(p.active);
      })
      .catch(() => undefined);
    void recordingConsent().then(setConsent).catch(() => undefined);
    void learningStatus()
      .then((s) => setCanReset(s.canReset))
      .catch(() => undefined);
  }

  useEffect(load, []);

  async function changeWho(name: string): Promise<void> {
    if (!name) return;
    setError(null);
    try {
      await setVoiceProfile(name);
      setWho(name);
      if (!profiles.includes(name)) setProfiles([...profiles, name]);
      // Consent, and whether there is a shipped version to fall back on, both belong to
      // the person — so they are re-read for whoever this now is.
      load();
      onProfileChange?.();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function changeConsent(on: boolean): Promise<void> {
    setError(null);
    try {
      await setRecordingConsent(on);
      setConsent(on);
      onProfileChange?.();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function forgetAll(): Promise<void> {
    if (
      !window.confirm(
        `Delete every recording of ${who} from this machine? This cannot be undone, and it also turns recording off for them.`,
      )
    ) {
      return;
    }
    setError(null);
    try {
      const gone = await forgetRecordings();
      setConsent(false);
      onProfileChange?.();
      window.alert(
        gone === 0 ? `There was nothing recorded for ${who}.` : `Deleted ${gone} recording${gone === 1 ? "" : "s"} of ${who}.`,
      );
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function addWho(): Promise<void> {
    const name = window.prompt("Name of the speaker (e.g. “Guest: Pastor Mensah”)")?.trim();
    if (name) await changeWho(name);
  }

  async function removeWho(): Promise<void> {
    if (PROTECTED_PROFILES.includes(who)) return;
    // Say what this actually does: removing a speaker deletes their recordings too.
    if (
      !window.confirm(
        `Remove “${who}”, their learned settings and every recording of them on this machine? This cannot be undone.`,
      )
    ) {
      return;
    }
    setError(null);
    try {
      await removeVoiceProfile(who);
      load();
      // The app has fallen back to another speaker: the panels showing this one's
      // services and proposal are about somebody who is no longer here.
      onProfileChange?.();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  /** The floor under all local learning: whatever this build shipped for this speaker. */
  async function resetToShipped(): Promise<void> {
    if (
      !window.confirm(
        `Put ${who} back exactly as the app shipped, discarding everything this machine has learnt about them?`,
      )
    ) {
      return;
    }
    setError(null);
    try {
      await resetProfileToBaked();
      load();
      onProfileChange?.();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  const protectedProfile = PROTECTED_PROFILES.includes(who);

  return (
    <details className="rounded-lg border" style={{ borderColor: "var(--border)" }}>
      <summary className="cursor-pointer px-3 py-2 text-sm font-medium">
        Voice profiles <span className="text-[var(--muted)]">· {who || "speaker"}</span>
      </summary>

      <div className="space-y-3 px-3 pb-3">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm font-medium">Preaching today:</span>
          <select
            className="select"
            style={{ width: "auto", minWidth: "12rem" }}
            value={who}
            disabled={disabled}
            onChange={(e) => void changeWho(e.target.value)}
          >
            {profiles.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>
          <button className="btn" disabled={disabled} onClick={() => void addWho()}>
            + Add
          </button>
          <button
            className="btn"
            disabled={disabled || protectedProfile}
            onClick={() => void removeWho()}
          >
            Remove
          </button>
        </div>

        {/* Recording a preacher is their call, not the app's. Off until asked for,
            asked per speaker, and undoable — including the recordings themselves. */}
        <div className="rounded-lg border p-2" style={{ borderColor: "var(--border)" }}>
          <label className="flex items-start gap-2 text-sm">
            <input
              type="checkbox"
              className="mt-0.5"
              checked={consent}
              disabled={disabled || !who}
              onChange={(e) => void changeConsent(e.target.checked)}
            />
            <span>
              Record services to improve {who || "this speaker"}
              {/* Not a description of a control: this is what the person ticking the
                  box is consenting to on someone else's behalf. Kept short, kept. */}
              <span className="block text-xs text-[var(--muted)]">
                Kept on this machine, never uploaded, deletable any time.
              </span>
            </span>
          </label>
          <div className="mt-1.5 flex flex-wrap gap-1.5">
            <button
              className="btn btn-sm"
              disabled={disabled || !who}
              onClick={() => void forgetAll()}
            >
              Delete {who || "this speaker"}’s recordings
            </button>
            {canReset && (
              <button
                className="btn btn-sm"
                disabled={disabled || !who}
                onClick={() => void resetToShipped()}
              >
                Reset {who} to as shipped
              </button>
            )}
          </div>
        </div>

        {error && <p className="text-sm text-red-500">{error}</p>}

        <LearnFromSermon model={model} who={who} disabled={disabled} />
      </div>
    </details>
  );
}
