import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  getProjection,
  getProjectionSettings,
  type ProjectionSettings,
  type ProjectionState,
} from "./api";
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

  useEffect(() => {
    getProjection().then(setState).catch(() => setState({ kind: "blank" }));
    getProjectionSettings().then(setSettings).catch(() => {});
    const subs = [
      listen<ProjectionState>("set-projection", (e) => setState(e.payload)),
      listen<ProjectionSettings>("set-settings", (e) => setSettings(e.payload)),
    ];
    return () => {
      subs.forEach((u) => u.then((f) => f()));
    };
  }, []);

  const theme = settings.theme;
  const scale = settings.fontScale || 1;
  const now = useNow(state.kind === "countdown");

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
      <div className="relative z-10 flex flex-col items-center justify-center">{body()}</div>
    </div>
  );
}
