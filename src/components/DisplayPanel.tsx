import { useEffect, useState } from "react";
import {
  blankProjection,
  clearAlert,
  getTicker,
  setTicker,
  getProjectionSettings,
  setFontScale,
  setProjection,
  setStageMessage,
  setStageTimer,
  showAlert,
  type ProjectionSettings,
} from "../api";
import { defaultProjectionSettings } from "../lib/themeDefaults";

/**
 * The display controls an operator reaches for *during* a service. Setting up
 * where the projection goes (stage window, phone remote, OBS/NDI) lives in
 * OutputsPanel on the Prepare tab, because that is done once, not mid-sermon.
 */
export function DisplayPanel() {
  const [message, setMessage] = useState("");
  const [stageMsg, setStageMsg] = useState("");
  const [minutes, setMinutes] = useState(5);
  const [label, setLabel] = useState("Starting soon");
  const [alertText, setAlertText] = useState("");
  // The announcement crawl. Read back on mount so reopening the console shows what
  // is actually running rather than an empty box over a live ticker.
  const [ticker, setTickerText] = useState("");
  const [tickerSecs, setTickerSecs] = useState(30);
  const [tickerStill, setTickerStill] = useState(false);
  useEffect(() => {
    void getTicker()
      .then((t) => {
        setTickerText(t.text);
        setTickerSecs(t.seconds || 30);
        setTickerStill(t.still);
      })
      .catch(() => undefined);
  }, []);
  const [alertSecs, setAlertSecs] = useState(10);
  const [settings, setSettings] = useState<ProjectionSettings>(defaultProjectionSettings);

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

      {/* The three an operator reaches for without looking. They stay put, always
          in the same place, because the moment you need Blank is never the moment
          to go hunting for it. */}
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
      </div>


      {/* Everything else here is occasional. It was all flat before - sixteen
          controls in one card - which made the panic buttons above harder to find
          than the things nobody presses twice a month. */}
      <details className="rounded-lg border" style={{ borderColor: "var(--border)" }}>
        <summary className="cursor-pointer px-3 py-2 text-sm font-medium">
          Show a message<span className="text-[var(--muted)]"> · full screen</span>
        </summary>
        <div className="space-y-3 px-3 pb-3">
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

        </div>
      </details>

      <details className="rounded-lg border" style={{ borderColor: "var(--border)" }}>
        <summary className="cursor-pointer px-3 py-2 text-sm font-medium">
          Announce<span className="text-[var(--muted)]"> · lower-third alert, or a crawl under the service</span>
        </summary>
        <div className="space-y-3 px-3 pb-3">
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
        />
        <span className="text-sm text-[var(--muted)]">s</span>
        <button
          onClick={() => alertText.trim() && showAlert(alertText.trim(), alertSecs)}
          className="btn btn-primary"
        >
          Alert
        </button>
        <button onClick={() => clearAlert()} className="btn">
          Clear
        </button>
      </div>

      {/* The announcement crawl. Unlike an alert it has no timeout: it runs under
          the welcome for as long as the operator wants and stops when they say so. */}

      <div className="flex flex-wrap items-center gap-2">
        <input
          value={ticker}
          onChange={(e) => setTickerText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void setTicker(ticker, tickerSecs, tickerStill);
          }}
          placeholder="Announcement crawl (runs under the live verse/song)"
          className="input flex-1"
        />
        <input
          type="number"
          min={5}
          max={120}
          value={tickerSecs}
          onChange={(e) => setTickerSecs(Math.min(120, Math.max(5, Number(e.target.value) || 30)))}
          className="input w-16 text-center"
        />
        <span className="text-sm text-[var(--muted)]">s</span>
        <label className="flex items-center gap-1 text-sm text-[var(--muted)]">
          <input
            type="checkbox"
            checked={tickerStill}
            onChange={(e) => setTickerStill(e.target.checked)}
          />
          Still
        </label>
        <button
          onClick={() => void setTicker(ticker, tickerSecs, tickerStill)}
          className="btn btn-primary"
        >
          Crawl
        </button>
        <button
          onClick={() => {
            setTickerText("");
            void setTicker("", tickerSecs, tickerStill);
          }}
          className="btn"
        >
          Stop
        </button>
      </div>

        </div>
      </details>

      <details className="rounded-lg border" style={{ borderColor: "var(--border)" }}>
        <summary className="cursor-pointer px-3 py-2 text-sm font-medium">
          Stage monitor<span className="text-[var(--muted)]"> · private note and timers, seen only by the platform</span>
        </summary>
        <div className="space-y-3 px-3 pb-3">
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
        <span className="text-sm text-[var(--muted)]">Stage timer</span>
        <button onClick={() => setStageTimer("countup", 0)} className="btn">
          Start elapsed
        </button>
        <button
          onClick={() => setStageTimer("countdown", minutes * 60)}
          className="btn"
          title={`Count down ${minutes} min on the stage monitor`}
        >
          Count down {minutes}m
        </button>
        <button onClick={() => setStageTimer("off", 0)} className="btn">
          Stop
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

        </div>
      </details>

      <details className="rounded-lg border" style={{ borderColor: "var(--border)" }}>
        <summary className="cursor-pointer px-3 py-2 text-sm font-medium">
          Text size<span className="text-[var(--muted)]"> · how large the words are on the wall</span>
        </summary>
        <div className="space-y-3 px-3 pb-3">
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
          <span className="text-xs text-[var(--muted)]">Backgrounds are under Setup → Themes</span>
        </div>
        </div>
      </details>
    </section>
  );
}
