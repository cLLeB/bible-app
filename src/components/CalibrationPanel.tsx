import { useEffect, useState } from "react";
import { LearnFromSermon } from "./LearnFromSermon";
import {
  PROTECTED_PROFILES,
  removeVoiceProfile,
  setVoiceProfile,
  voiceProfiles,
  type SttModel,
} from "../api";

interface CalibrationPanelProps {
  model: SttModel;
  disabled: boolean;
}

/**
 * Pick who is preaching, and teach the app new speakers from their sermons. Settings
 * are stored per speaker, so each is tuned on their own. The President and
 * Vice-President are baked in and can't be removed.
 */
export function CalibrationPanel({ model, disabled }: CalibrationPanelProps) {
  const [profiles, setProfiles] = useState<string[]>([]);
  const [who, setWho] = useState<string>("");
  const [error, setError] = useState<string | null>(null);

  function load(): void {
    void voiceProfiles()
      .then((p) => {
        setProfiles(p.all);
        setWho(p.active);
      })
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
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function addWho(): Promise<void> {
    const name = window.prompt("Name of the speaker (e.g. “Guest — Pastor Mensah”)")?.trim();
    if (name) await changeWho(name);
  }

  async function removeWho(): Promise<void> {
    if (PROTECTED_PROFILES.includes(who)) return;
    if (!window.confirm(`Remove the “${who}” profile and its learned settings?`)) return;
    setError(null);
    try {
      await removeVoiceProfile(who);
      load();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  const protectedProfile = PROTECTED_PROFILES.includes(who);

  return (
    <details className="rounded-lg border" style={{ borderColor: "var(--border)" }}>
      <summary className="cursor-pointer px-3 py-2 text-sm font-medium">
        Voice profiles <span className="text-[var(--muted)]">— {who || "speaker"}</span>
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
            title="Whose voice the app is tuned for"
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
            title={protectedProfile ? "Baked-in preachers can't be removed" : "Remove this profile"}
          >
            Remove
          </button>
        </div>

        <p className="text-sm text-[var(--muted)]">
          Each speaker is tuned on their own — teaching one never affects the others.
          Everything stays on this machine.
        </p>

        {error && <p className="text-sm text-red-500">{error}</p>}

        <LearnFromSermon model={model} who={who} disabled={disabled} />
      </div>
    </details>
  );
}
