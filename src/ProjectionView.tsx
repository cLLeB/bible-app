import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  getProjection,
  getProjectionSettings,
  type ProjectionSettings,
  type ProjectionState,
} from "./api";

const THEMES: Record<string, { bg: string; fg: string; sub: string }> = {
  dark: { bg: "#000000", fg: "#ffffff", sub: "#c9c9c9" },
  light: { bg: "#ffffff", fg: "#101010", sub: "#555555" },
  sepia: { bg: "#f4ecd8", fg: "#5b4636", sub: "#8a725a" },
};

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
  const [settings, setSettings] = useState<ProjectionSettings>({ fontScale: 1, theme: "dark" });

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

  const theme = THEMES[settings.theme] ?? THEMES.dark;
  const scale = settings.fontScale || 1;
  const now = useNow(state.kind === "countdown");

  // Auto-fit: shrink the text as the verse gets longer so it never overflows.
  function fitRem(text: string): number {
    const n = text.length;
    const base = n < 120 ? 3.2 : n < 220 ? 2.6 : n < 340 ? 2.1 : n < 500 ? 1.7 : 1.4;
    return base * scale;
  }
  const bodyText = state.kind === "verse" || state.kind === "song" || state.kind === "message" ? state.text : "";
  const bodyStyle = { fontSize: `${fitRem(bodyText)}rem`, lineHeight: 1.15 };
  const capStyle = { fontSize: `${1.4 * scale}rem`, color: theme.sub };

  function body() {
    switch (state.kind) {
      case "verse":
      case "song":
        return (
          <>
            <p className="mb-8 max-w-6xl whitespace-pre-line" style={bodyStyle}>
              {state.text}
            </p>
            <p style={capStyle}>{state.caption}</p>
          </>
        );
      case "message":
        return (
          <p className="max-w-6xl whitespace-pre-line" style={bodyStyle}>
            {state.text}
          </p>
        );
      case "countdown":
        return (
          <>
            <p style={{ fontSize: `${5 * scale}rem`, fontVariantNumeric: "tabular-nums" }}>
              {formatRemaining(state.targetMs - now)}
            </p>
            {state.label && <p style={capStyle}>{state.label}</p>}
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
        return <p style={{ fontSize: "0.9rem", color: theme.sub }}>Projection ready</p>;
    }
  }

  return (
    <div
      className="flex h-screen w-screen flex-col items-center justify-center px-16 text-center"
      style={{ backgroundColor: state.kind === "blackout" ? "#000000" : theme.bg, color: theme.fg }}
    >
      {body()}
    </div>
  );
}
