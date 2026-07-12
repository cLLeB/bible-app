import { useEffect, useState } from "react";
import {
  blankProjection,
  getProjectionSettings,
  setProjection,
  setProjectionSettings,
  showStage,
  type ProjectionSettings,
} from "../api";

export function DisplayPanel() {
  const [message, setMessage] = useState("");
  const [minutes, setMinutes] = useState(5);
  const [label, setLabel] = useState("Starting soon");
  const [settings, setSettings] = useState<ProjectionSettings>({ fontScale: 1, theme: "dark" });

  useEffect(() => {
    getProjectionSettings().then(setSettings).catch(() => {});
  }, []);

  function applySettings(next: ProjectionSettings): void {
    setSettings(next);
    void setProjectionSettings(next);
  }

  return (
    <section className="space-y-3">
      <h2 className="text-xl font-semibold">Display</h2>

      <div className="flex flex-wrap gap-2">
        <button onClick={() => blankProjection()} className="rounded border px-3 py-1">
          Blank
        </button>
        <button
          onClick={() => setProjection({ kind: "blackout" })}
          className="rounded bg-black px-3 py-1 text-white"
        >
          Blackout
        </button>
        <button onClick={() => setProjection({ kind: "logo" })} className="rounded border px-3 py-1">
          Logo
        </button>
        <button onClick={() => showStage()} className="rounded border px-3 py-1">
          Stage display
        </button>
      </div>

      <div className="flex gap-2">
        <input
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          placeholder="On-screen message / announcement"
          className="flex-1 rounded border px-2 py-1"
        />
        <button
          onClick={() => message.trim() && setProjection({ kind: "message", text: message.trim() })}
          className="rounded bg-blue-600 px-3 py-1 text-white"
        >
          Show
        </button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <span className="text-sm">Countdown</span>
        <input
          type="number"
          min={1}
          max={120}
          value={minutes}
          onChange={(e) => setMinutes(Math.max(1, Number(e.target.value) || 1))}
          className="w-16 rounded border px-2 py-1"
        />
        <span className="text-sm">min ·</span>
        <input
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          placeholder="label"
          className="w-40 rounded border px-2 py-1"
        />
        <button
          onClick={() =>
            setProjection({ kind: "countdown", targetMs: Date.now() + minutes * 60_000, label })
          }
          className="rounded bg-blue-600 px-3 py-1 text-white"
        >
          Start
        </button>
      </div>

      <div className="flex flex-wrap items-center gap-4 border-t pt-2">
        <label className="flex items-center gap-2 text-sm">
          Font
          <input
            type="range"
            min={0.6}
            max={2}
            step={0.1}
            value={settings.fontScale}
            onChange={(e) => applySettings({ ...settings, fontScale: Number(e.target.value) })}
          />
          {Math.round(settings.fontScale * 100)}%
        </label>
        <label className="flex items-center gap-2 text-sm">
          Theme
          <select
            value={settings.theme}
            onChange={(e) =>
              applySettings({ ...settings, theme: e.target.value as ProjectionSettings["theme"] })
            }
            className="rounded border px-2 py-1"
          >
            <option value="dark">Dark</option>
            <option value="light">Light</option>
            <option value="sepia">Sepia</option>
          </select>
        </label>
      </div>
    </section>
  );
}
