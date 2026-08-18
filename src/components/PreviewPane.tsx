import { useEffect, useRef, useState } from "react";
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
import { applyOutput, laptopOutput, listOutputs } from "../lib/audioSink";
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
/**
 * How wide the preview box may get, by what is in it.
 *
 * A preview is a check, not a second congregation screen. Text only has to be
 * legible enough to confirm the reference and the wording, which takes less room
 * than a picture does to recognise; a video wants a little more again because the
 * operator is watching it rather than reading it. Capped in every case, because at
 * full column width the preview pushes the run sheet and the transport off the
 * bottom of the window.
 */
function previewWidth(kind: string): string {
  switch (kind) {
    case "video":
      return "34rem";
    case "image":
      return "30rem";
    default:
      // Verses, songs, messages, countdowns: words on a background.
      return "26rem";
  }
}

export function PreviewPane() {
  const { staged, clear } = usePreviewStore();
  // Read off the clip itself once its metadata arrives. Null until then, and reset
  // whenever the staged item changes so a stale length is never shown against a new
  // clip.
  const [duration, setDuration] = useState<number | null>(null);
  // The preview clip itself, so extra transport can reach it without remounting.
  const clipRef = useRef<HTMLVideoElement | null>(null);
  const [rate, setRate] = useState(1);
  const [hushed, setHushed] = useState(true);
  // This machine's own speakers, found once and held. The preview is pinned to them
  // by name rather than to "the system default": the default is whatever Windows
  // says, and a church that once made the TV their default would have the preview
  // talking through the hall.
  const [ownSpeakers, setOwnSpeakers] = useState("");
  useEffect(() => {
    void listOutputs()
      .then((outs) => setOwnSpeakers(laptopOutput(outs)))
      .catch(() => undefined);
  }, []);
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

  // A length read from the previous clip must never be shown against this one.
  useEffect(() => {
    setDuration(null);
    setRate(1);
    setHushed(true);
  }, [staged?.kind === "video" ? staged.src : null]);

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

  /** Step the preview clip, clamped so it cannot run off either end. */
  function nudge(seconds: number): void {
    const el = clipRef.current;
    if (!el) return;
    const end = Number.isFinite(el.duration) ? el.duration : 0;
    el.currentTime = Math.min(Math.max(0, el.currentTime + seconds), end || el.currentTime);
  }

  /** Cycle the speed. Half speed for reading a caption, double for scanning. */
  function cycleRate(): void {
    const order = [1, 1.5, 2, 0.5];
    const next = order[(order.indexOf(rate) + 1) % order.length];
    setRate(next);
    if (clipRef.current) clipRef.current.playbackRate = next;
  }


  return (
    <section className="w-fit max-w-full space-y-2">
      <div className="flex w-full flex-wrap items-center gap-2">
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

      {/* Sized to what is in it, not to a fixed rectangle.
          A preview is a check, not a second congregation screen. It used to be a
          16:9 letterbox whatever it held, so a portrait photo sat between two black
          bars and a two-line verse sat in a mostly empty box - and the empty space
          pushed the transport and the run sheet down the window for no gain. Media
          now sets the height from its own shape, and text from how much of it there
          is. */}
      <div
        className="relative overflow-hidden rounded border"
        style={{
          maxWidth: previewWidth(staged.kind),
          borderColor: "var(--border)",
          background: dark ? "#000000" : backgroundCss(theme),
          // Only a floor, and only so a blackout - which has nothing in it at all -
          // is still a visible thing on the screen rather than a hairline.
          minHeight: "4rem",
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

        {/* In normal flow, so the picture's own proportions decide the box. */}
        {staged.kind === "image" && (
          <img
            src={needsAssetUrl(staged.src) ? convertFileSrc(staged.src) : staged.src}
            alt=""
            className="block h-auto w-full"
            style={{ background: "#000000" }}
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
          //
          // In normal flow too, so a 4:3 clip is not letterboxed into 16:9.
          <video
            ref={clipRef}
            src={convertFileSrc(staged.src)}
            className="block h-auto w-full"
            style={{ background: "#000000" }}
            muted={hushed}
            controls
            playsInline
            preload="metadata"
            onLoadedMetadata={(e) => {
              setDuration(e.currentTarget.duration);
              // A preview belongs on the machine the operator is sitting at, whatever
              // the congregation screen has been pointed at. Pinned to this laptop's
              // own speakers by name; an empty id means none could be identified, and
              // the system default is the only remaining answer.
              void applyOutput(e.currentTarget, ownSpeakers);
            }}
          />
        )}

        {/* Words also flow, so a short verse gives a short box. The caption comes
            after the body rather than pinned to the bottom, which it has to now that
            there is no fixed height to pin it against. */}
        {!lines.visual && (
          <div className="flex flex-col items-center gap-1 px-[6%] py-4 text-center">
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
            {lines.caption && (
              <p
                className="truncate max-w-full"
                style={{ ...captionStyle(theme, 1), fontSize: "0.7rem" }}
              >
                {lines.caption}
              </p>
            )}
          </div>
        )}

        {/* Over a picture or a clip the caption still sits on the media, where it
            reads as a label rather than as part of the slide. */}
        {lines.visual && lines.caption && (
          <p
            className="absolute inset-x-0 bottom-1 truncate px-2 text-center"
            style={{ ...captionStyle(theme, 1), fontSize: "0.7rem", color: "#d0d0d0" }}
          >
            {lines.caption}
          </p>
        )}
      </div>
      {staged.kind === "video" && (
        // Beyond what the browser's own controls offer. Skipping in fives is how an
        // operator finds the bit they half remember; the speed is for scanning a long
        // clip without watching all of it; sound is off to begin with because a
        // preview that starts talking over the service would be worse than no
        // preview, and on demand because half of checking a clip is hearing it.
        <div className="flex flex-wrap items-center gap-1.5 text-sm">
          <button className="btn btn-sm" onClick={() => nudge(-5)} title="Back 5 seconds">
            ⏴ 5s
          </button>
          <button className="btn btn-sm" onClick={() => nudge(5)} title="Forward 5 seconds">
            5s ⏵
          </button>
          <button className="btn btn-sm" onClick={cycleRate} title="Playback speed">
            {rate}×
          </button>
          <button
            className={`btn btn-sm ${hushed ? "" : "btn-primary"}`}
            onClick={() => setHushed((v) => !v)}
            title="Preview sound plays on this laptop, never on the chosen output"
          >
            {hushed ? "Silent" : "Sound"}
          </button>
          <span className="text-xs text-[var(--faint)]">on this laptop only</span>
        </div>
      )}
    </section>
  );
}
