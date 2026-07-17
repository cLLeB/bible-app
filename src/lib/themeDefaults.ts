import type { ProjectionSettings, Theme } from "../api";

/**
 * Frontend fallback that mirrors the Rust `dark` built-in (see
 * `src-tauri/src/themes.rs`). Used only as the initial value before the backend's
 * real settings arrive, so the projection screen is never un-styled.
 */
export const DEFAULT_THEME: Theme = {
  id: "dark",
  name: "Dark",
  background: { kind: "color", color: "#000000", color2: "#000000", angle: 0 },
  text: {
    fontFamily: "Inter, 'Segoe UI', system-ui, sans-serif",
    color: "#ffffff",
    captionColor: "#c9c9c9",
    align: "center",
    weight: 400,
    shadow: false,
    uppercase: false,
  },
  builtIn: true,
};

export const defaultProjectionSettings: ProjectionSettings = {
  fontScale: 1,
  theme: DEFAULT_THEME,
};
