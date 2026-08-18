import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  getProjectionSettings,
  setProjection,
  type ProjectionSettings,
  type ProjectionState,
} from "../api";
import { backgroundCss, bodyStyle, captionStyle, mediaBackground } from "../lib/theme";
import { previewLabel, previewLines } from "../lib/preview";
import { needsAssetUrl } from "../lib/projection";
import { defaultProjectionSettings } from "../lib/themeDefaults";
import { usePreviewStore } from "../services";

/**
 * What the wall will look like, before it looks like that.
 *
 * The console already previewed scripture as *text* (Shift+Enter in the lookup
 * bar) and previewed a theme in the theme editor. Neither answers the question
 * an operator actually has thirty seconds before a cue: does this item, in the
 * theme that is live right now, read from the back of the room. So this renders
 * through the very same helpers as the projection window, and the file
 * containing them says so in its own doc comment: what the operator previews is
 * exactly what the congregation sees.
 *
 * Nothing here touches the projection until Go live is pressed.
 */
export function PreviewPane() {
  const { staged, clear } = usePreviewStore();
  // Read off the clip itself once its metadata arrives. Null until then, and reset
  // whenever the staged item changes so a stale length is never shown against a new
  // clip.
  const [duration, setDuration] = useState<number | null>(null);
  const [settings, setSettings] = useState<ProjectionSettings>(defaultProjectionSettings);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getProjectionSettings().then(setSettings).catch(() => {});
    // Follow the live theme: previewing in last week's look would be a lie.
    const sub = listen<ProjectionSettings>("set-settings", (e) => setSettings(e.payload));
    return () => {
      sub.then((f) => f());
    };
  }, []);

  if (!staged) return null;

  const theme = settings.theme;
  const lines = previewLines(staged);
  const media = mediaBackground(theme);
  const dark = staged.kind === "blackout";

  async function goLive(next: ProjectionState): Promise<void> {
    setError(null);
    try {
      await setProjection(next);
      clear();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  // A length read from the previous clip must never be shown against this one.
  useEffect(() => {
    setDuration(null);
  }, [staged?.kind === "video" ? staged.src : null]);

  return (
    <section className="space-y-2">
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="panel-title">Preview</h2>
        <span className="truncate text-xs text-[var(--faint)]">{previewLabel(staged)}</span>
        {staged.kind === "video" && (
          // The three things worth knowing before a clip reaches the wall. Length is
          // read off the file; the other two are how it will actually be played, and
          // both have surprised an operator before now.
          <span className="text-xs text-[var(--muted)]">
            {duration !== null && `${Math.floor(duration / 60)}:${String(Math.round(duration % 60)).padStart(2, "0")}`}
            {staged.muted ? " · silent" : " · with sound"}
            {staged.looping ? " · loops" : ""}
          </span>
        )}
        <div className="ml-auto flex items-center gap-2">
          <button onClick={() => void goLive(staged)} className="btn btn-sm btn-primary">
            Go live
          </button>
          <button onClick={clear} className="btn btn-sm" title="Clear the preview">
            ✕
          </button>
        </div>
      </div>

      {error && <p className="tint tint-bad rounded px-2 py-1 text-sm">{error}</p>}

      {/* A 16:9 still of the congregation screen. Video previews as its first
          frame rather than playing: two copies of the same clip running out of
          step is a distraction at exactly the wrong moment. */}
      <div
        className="relative w-full overflow-hidden rounded border"
        style={{
          aspectRatio: "16 / 9",
          borderColor: "var(--border)",
          background: dark ? "#000000" : backgroundCss(theme),
        }}
      >
        {!dark && media && media.kind === "image" && (
          <img
            src={convertFileSrc(media.src)}
            alt=""
            className="absolute inset-0 h-full w-full"
            style={{ objectFit: media.fit }}
          />
        )}
        {!dark && media && media.dim > 0 && (
          <div
            className="absolute inset-0"
            style={{ background: `rgba(0,0,0,${media.dim})` }}
          />
        )}

        {staged.kind === "image" && (
          <img
            src={needsAssetUrl(staged.src) ? convertFileSrc(staged.src) : staged.src}
            alt=""
            className="absolute inset-0 h-full w-full"
            style={{ objectFit: "contain", background: "#000000" }}
          />
        )}
        {staged.kind === "video" && (
          // Scrubbable and playable, muted, with its own controls.
          //
          // A single frozen frame was nearly useless: the first frame of a clip is
          // usually black or a title card, so it answered neither "is this the right
          // one" nor "what happens when it starts". Controls here are safe in a way
          // they never are on the wall - this pane is the operator's own screen, and
          // the point of a preview is to find out before the congregation does.
          <video
            src={convertFileSrc(staged.src)}
            className="absolute inset-0 h-full w-full"
            style={{ objectFit: "contain", background: "#000000" }}
            muted
            controls
            playsInline
            preload="metadata"
            onLoadedMetadata={(e) => setDuration(e.currentTarget.duration)}
          />
        )}

        {!lines.visual && (
          <div className="absolute inset-0 flex flex-col items-center justify-center px-[6%] text-center">
            {lines.body && (
              <p
                className="whitespace-pre-line"
                style={{
                  ...bodyStyle(theme, lines.body.length, settings.fontScale || 1),
                  // The pane is a fraction of the wall, so the theme's sizing is
                  // scaled to match rather than overflowing the box.
                  fontSize: `${Math.max(0.6, 1.5 - lines.body.length / 400)}rem`,
                  lineHeight: 1.2,
                }}
              >
                {lines.body}
              </p>
            )}
          </div>
        )}

        {lines.caption && (
          <p
            className="absolute inset-x-0 bottom-1 truncate px-2 text-center"
            style={{
              ...captionStyle(theme, 1),
              fontSize: "0.7rem",
              // Visual items sit on their own black letterbox, so the caption
              // needs a colour that survives it rather than the theme's.
              color: lines.visual ? "#d0d0d0" : captionStyle(theme, 1).color,
            }}
          >
            {lines.caption}
          </p>
        )}
      </div>
    </section>
  );
}
