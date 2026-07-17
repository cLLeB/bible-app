import { useEffect, useState } from "react";
import {
  blankProjection,
  clearAlert,
  getProjectionSettings,
  setFontScale,
  setProjection,
  setStageMessage,
  showAlert,
  showStage,
  startRemote,
  type ProjectionSettings,
} from "../api";
import { defaultProjectionSettings } from "../lib/themeDefaults";

export function DisplayPanel() {
  const [message, setMessage] = useState("");
  const [stageMsg, setStageMsg] = useState("");
  const [minutes, setMinutes] = useState(5);
  const [label, setLabel] = useState("Starting soon");
  const [alertText, setAlertText] = useState("");
  const [alertSecs, setAlertSecs] = useState(10);
  const [settings, setSettings] = useState<ProjectionSettings>(defaultProjectionSettings);
  const [remoteUrl, setRemoteUrl] = useState<string | null>(null);

  useEffect(() => {
    getProjectionSettings().then(setSettings).catch(() => {});
  }, []);

  function changeFontScale(scale: number): void {
    setSettings((s) => ({ ...s, fontScale: scale }));
    void setFontScale(scale);
  }

  return (
    <section className="space-y-3">
      <h2 className="panel-title">Display</h2>

      <div className="flex flex-wrap gap-2">
        <button onClick={() => blankProjection()} className="btn">
          Blank
        </button>
        <button onClick={() => setProjection({ kind: "blackout" })} className="btn btn-dark">
          Blackout
        </button>
        <button onClick={() => setProjection({ kind: "logo" })} className="btn">
          Logo
        </button>
        <button onClick={() => showStage()} className="btn">
          Stage display
        </button>
        <button onClick={async () => setRemoteUrl(await startRemote())} className="btn">
          Phone remote
        </button>
      </div>

      {remoteUrl && (
        <div className="space-y-1 text-sm text-gray-600">
          <p>
            Phone remote: open <span className="font-mono text-blue-700">{remoteUrl}</span> on a
            phone on the same Wi-Fi.
          </p>
          <p>
            OBS / browser output: add a Browser Source at{" "}
            <span className="font-mono text-blue-700">{remoteUrl}/projection</span>.
          </p>
        </div>
      )}

      <div className="flex gap-2">
        <input
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          placeholder="On-screen message / announcement"
          className="input flex-1"
        />
        <button
          onClick={() => message.trim() && setProjection({ kind: "message", text: message.trim() })}
          className="btn btn-primary"
        >
          Show
        </button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <input
          value={alertText}
          onChange={(e) => setAlertText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && alertText.trim()) void showAlert(alertText.trim(), alertSecs);
          }}
          placeholder="Lower-third alert (shows over the live verse/song)"
          className="input flex-1"
        />
        <input
          type="number"
          min={0}
          max={300}
          value={alertSecs}
          onChange={(e) => setAlertSecs(Math.max(0, Number(e.target.value) || 0))}
          className="input w-16 text-center"
          title="Auto-dismiss after this many seconds (0 = stays until cleared)"
        />
        <span className="text-sm text-[var(--muted)]">s</span>
        <button
          onClick={() => alertText.trim() && showAlert(alertText.trim(), alertSecs)}
          className="btn btn-primary"
          title="Overlays the congregation screen without hiding what's live"
        >
          Alert
        </button>
        <button onClick={() => clearAlert()} className="btn">
          Clear
        </button>
      </div>

      <div className="flex gap-2">
        <input
          value={stageMsg}
          onChange={(e) => setStageMsg(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && stageMsg.trim()) void setStageMessage(stageMsg.trim());
          }}
          placeholder="Private note to the stage (e.g. wrap up, go to prayer)"
          className="input flex-1"
        />
        <button
          onClick={() => stageMsg.trim() && setStageMessage(stageMsg.trim())}
          className="btn btn-primary"
          title="Shows only on the stage monitor, never on the projector"
        >
          Send
        </button>
        <button
          onClick={() => {
            setStageMsg("");
            void setStageMessage("");
          }}
          className="btn"
        >
          Clear
        </button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <span className="text-sm text-[var(--muted)]">Countdown</span>
        <input
          type="number"
          min={1}
          max={120}
          value={minutes}
          onChange={(e) => setMinutes(Math.max(1, Number(e.target.value) || 1))}
          className="input w-16 text-center"
        />
        <span className="text-sm text-[var(--muted)]">min ·</span>
        <input
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          placeholder="label"
          className="input w-40"
        />
        <button
          onClick={() =>
            setProjection({ kind: "countdown", targetMs: Date.now() + minutes * 60_000, label })
          }
          className="btn btn-primary"
        >
          Start
        </button>
      </div>

      <div className="flex flex-wrap items-center gap-x-5 gap-y-2 border-t pt-3">
        <label className="flex items-center gap-2 text-sm">
          Font
          <input
            type="range"
            min={0.6}
            max={2}
            step={0.1}
            value={settings.fontScale}
            onChange={(e) => changeFontScale(Number(e.target.value))}
          />
          <span className="tabular-nums text-[var(--muted)]">{Math.round(settings.fontScale * 100)}%</span>
        </label>
        <span className="text-xs text-[var(--muted)]">Theme &amp; backgrounds → Themes panel</span>
      </div>
    </section>
  );
}
