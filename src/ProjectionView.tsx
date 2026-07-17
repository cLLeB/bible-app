import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  getAlert,
  getProjection,
  getProjectionSettings,
  type Alert,
  type ProjectionSettings,
  type ProjectionState,
} from "./api";
import { alertVisible } from "./lib/alert";
import { backgroundCss, bodyStyle, captionStyle, mediaBackground } from "./lib/theme";
import { defaultProjectionSettings } from "./lib/themeDefaults";

function useNow(active: boolean): number {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!active) return;
    const id = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(id);
  }, [active]);
  return now;
}

function formatRemaining(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function ProjectionView() {
  const [state, setState] = useState<ProjectionState>({ kind: "blank" });
  const [settings, setSettings] = useState<ProjectionSettings>(defaultProjectionSettings);
  const [alert, setAlert] = useState<Alert>({ text: "", untilMs: 0 });

  useEffect(() => {
    getProjection().then(setState).catch(() => setState({ kind: "blank" }));
    getProjectionSettings().then(setSettings).catch(() => {});
    getAlert().then(setAlert).catch(() => {});
    const subs = [
      listen<ProjectionState>("set-projection", (e) => setState(e.payload)),
      listen<ProjectionSettings>("set-settings", (e) => setSettings(e.payload)),
      listen<Alert>("set-alert", (e) => setAlert(e.payload)),
    ];
    return () => {
      subs.forEach((u) => u.then((f) => f()));
    };
  }, []);

  const theme = settings.theme;
  const scale = settings.fontScale || 1;
  // Tick while a countdown runs or a timed alert is counting down to dismissal.
  const now = useNow(state.kind === "countdown" || (alert.text !== "" && alert.untilMs !== 0));
  const showAlert = state.kind !== "blackout" && alertVisible(alert, now);

  const bodyText = state.kind === "verse" || state.kind === "song" || state.kind === "message" ? state.text : "";
  const bodyCss = bodyStyle(theme, bodyText.length, scale);
  const capCss = captionStyle(theme, scale);

  function body() {
    switch (state.kind) {
      case "verse":
      case "song":
        return (
          <>
            <p className="mb-8 max-w-6xl whitespace-pre-line" style={bodyCss}>
              {state.text}
            </p>
            <p style={capCss}>{state.caption}</p>
          </>
        );
      case "parallel":
        return (
          <>
            <div className="grid w-full max-w-[92vw] grid-cols-1 gap-8 md:grid-cols-2">
              {[
                { text: state.primaryText, code: state.primaryCode },
                { text: state.secondaryText, code: state.secondaryCode },
              ].map((col, i) => (
                <div key={i} className="flex flex-col items-center">
                  <p
                    className="mb-3 whitespace-pre-line"
                    style={bodyStyle(theme, Math.max(state.primaryText.length, state.secondaryText.length) * 2, scale)}
                  >
                    {col.text || "—"}
                  </p>
                  <p style={{ ...capCss, fontSize: `${1 * scale}rem` }}>{col.code}</p>
                </div>
              ))}
            </div>
            <p className="mt-6" style={capCss}>
              {state.caption}
            </p>
          </>
        );
      case "message":
        return (
          <p className="max-w-6xl whitespace-pre-line" style={bodyCss}>
            {state.text}
          </p>
        );
      case "countdown":
        return (
          <>
            <p style={{ ...bodyCss, fontSize: `${5 * scale}rem`, fontVariantNumeric: "tabular-nums" }}>
              {formatRemaining(state.targetMs - now)}
            </p>
            {state.label && <p style={capCss}>{state.label}</p>}
          </>
        );
      case "logo":
        return (
          <img
            src="/newbreed_logo.png"
            alt="New Breed"
            style={{
              width: `${45 * scale}vw`,
              maxWidth: "40rem",
              height: "auto",
              background: "#fff",
              borderRadius: "1rem",
              padding: "1.5rem",
            }}
          />
        );
      case "blackout":
        return null;
      case "blank":
      default:
        return <p style={{ fontSize: "0.9rem", color: theme.text.captionColor }}>Projection ready</p>;
    }
  }

  // Media (image/video) backgrounds are hidden during blackout so the screen
  // goes truly dark.
  const media = state.kind === "blackout" ? null : mediaBackground(theme);

  // A stable signature of the *content* so the slide fades in only on real slide
  // changes — not on every countdown tick.
  const contentKey =
    state.kind === "verse" || state.kind === "song"
      ? `${state.kind}:${state.caption}:${state.text}`
      : state.kind === "parallel"
        ? `parallel:${state.caption}:${state.primaryCode}:${state.secondaryCode}`
        : state.kind === "message"
        ? `message:${state.text}`
        : state.kind === "countdown"
          ? `countdown:${state.label}`
          : state.kind;

  return (
    <div
      className="relative flex h-screen w-screen flex-col items-center justify-center overflow-hidden px-16 text-center"
      style={{
        background: state.kind === "blackout" ? "#000000" : backgroundCss(theme),
        color: theme.text.color,
      }}
    >
      {media && (
        <>
          {media.kind === "image" ? (
            <img
              src={convertFileSrc(media.src)}
              alt=""
              className="absolute inset-0 h-full w-full"
              style={{ objectFit: media.fit }}
            />
          ) : (
            <video
              src={convertFileSrc(media.src)}
              className="absolute inset-0 h-full w-full"
              style={{ objectFit: media.fit }}
              autoPlay
              loop
              muted
              playsInline
            />
          )}
          {media.dim > 0 && (
            <div className="absolute inset-0" style={{ background: `rgba(0,0,0,${media.dim})` }} />
          )}
        </>
      )}
      <div key={contentKey} className="proj-fade relative z-10 flex flex-col items-center justify-center">
        {body()}
      </div>

      {showAlert && (
        <div
          className="absolute inset-x-0 bottom-0 z-20 px-10 py-5 text-center"
          style={{
            background: "rgba(0,0,0,0.72)",
            color: "#ffffff",
            fontFamily: theme.text.fontFamily,
            fontSize: `${1.9 * scale}rem`,
            fontWeight: 600,
          }}
        >
          {alert.text}
        </div>
      )}
    </div>
  );
}
