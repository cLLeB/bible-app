import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  getAlert,
  getAudio,
  getAudioOutput,
  getProjection,
  getProjectionSettings,
  videoEnded,
  type Alert,
  type AudioOutput,
  type AudioState,
  type ProjectionSettings,
  type ProjectionState,
} from "./api";
import { alertVisible } from "./lib/alert";
import { applyOutput } from "./lib/audioSink";
import { coversScreen, needsAssetUrl } from "./lib/projection";
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

/** A source this window can actually load, whatever form it arrived in. */
function assetSrc(src: string): string {
  return needsAssetUrl(src) ? convertFileSrc(src) : src;
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
  const [audio, setAudio] = useState<AudioState>({
    src: "",
    title: "",
    paused: true,
    looping: false,
    volume: 1,
  });

  useEffect(() => {
    getProjection().then(setState).catch(() => setState({ kind: "blank" }));
    getProjectionSettings().then(setSettings).catch(() => {});
    getAlert().then(setAlert).catch(() => {});
    getAudio().then(setAudio).catch(() => {});
    const subs = [
      listen<ProjectionState>("set-projection", (e) => setState(e.payload)),
      listen<ProjectionSettings>("set-settings", (e) => setSettings(e.payload)),
      listen<Alert>("set-alert", (e) => setAlert(e.payload)),
      listen<AudioState>("set-audio", (e) => setAudio(e.payload)),
    ];
    return () => {
      subs.forEach((u) => u.then((f) => f()));
    };
  }, []);

  // The live <video>, so transport can reach it without re-mounting the element
  // (re-mounting would restart playback, which is the one thing a bumper must
  // not do when the operator only pressed Mute).
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const videoState = state.kind === "video" ? state : null;

  useEffect(() => {
    const el = videoRef.current;
    if (!el || !videoState) return;
    if (videoState.paused) {
      el.pause();
    } else {
      // Autoplay can be refused; a caught rejection keeps the window alive.
      void el.play().catch(() => {});
    }
  }, [videoState?.paused, videoState?.src]);

  useEffect(() => {
    const sub = listen<number>("video-seek", (e) => {
      const el = videoRef.current;
      if (el) el.currentTime = Math.max(0, e.payload) / 1000;
    });
    return () => {
      sub.then((f) => f());
    };
  }, []);

  // The live <audio>. It lives in this window rather than the console's because
  // this is the window that stays open for the whole service: reloading the
  // operator's console must never cut the music off.
  //
  // It is driven imperatively for the same reason the video is — re-mounting on
  // every state change would restart the track when the operator only nudged
  // the volume.
  const audioRef = useRef<HTMLAudioElement | null>(null);

  // Which speakers the sound goes to. Windows sends it to the system default
  // device whatever monitor this window is on, so projecting onto a TV over HDMI
  // otherwise leaves the sound in the laptop. An empty id means the default, which
  // is what every install had until this was added.
  const [soundOut, setSoundOut] = useState("");

  useEffect(() => {
    void getAudioOutput()
      .then((o) => setSoundOut(o.id))
      .catch(() => {});
    const sub = listen<AudioOutput>("set-audio-output", (e) => setSoundOut(e.payload.id));
    return () => {
      sub.then((f) => f());
    };
  }, []);

  // Re-applied whenever the choice changes or an element is remounted: a fresh
  // <video> starts out on the default device, so choosing the output once is not
  // enough — every clip has to be pointed at it again.
  useEffect(() => {
    void applyOutput(audioRef.current, soundOut);
    void applyOutput(videoRef.current, soundOut);
  }, [soundOut, audio.src, state]);

  useEffect(() => {
    const el = audioRef.current;
    if (!el || audio.src === "") return;
    el.volume = Math.min(1, Math.max(0, audio.volume));
    el.loop = audio.looping;
    if (audio.paused) {
      el.pause();
    } else {
      // Autoplay can be refused; a caught rejection keeps the window alive.
      void el.play().catch(() => {});
    }
  }, [audio.src, audio.paused, audio.looping, audio.volume]);

  useEffect(() => {
    const sub = listen<number>("audio-seek", (e) => {
      const el = audioRef.current;
      if (el) el.currentTime = Math.max(0, e.payload) / 1000;
    });
    return () => {
      sub.then((f) => f());
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
      case "image":
        return (
          <img
            src={assetSrc(state.src)}
            alt=""
            className="absolute inset-0 h-full w-full"
            style={{ objectFit: "contain" }}
          />
        );
      case "video":
        return (
          <video
            ref={videoRef}
            // Keyed on the file so choosing a different video mounts a fresh
            // element, while mute/loop/pause only update this one.
            key={state.src}
            src={assetSrc(state.src)}
            className="absolute inset-0 h-full w-full"
            style={{ objectFit: "contain", background: "#000000" }}
            autoPlay
            playsInline
            muted={state.muted}
            loop={state.looping}
            // Only the element knows how long the clip is. Telling the backend
            // lets an announcements loop move on at the video's own length
            // instead of cutting it off at an image's dwell time.
            onEnded={() => void videoEnded().catch(() => {})}
          />
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
  // goes truly dark, and behind full-screen media, which already covers them.
  // Decoding a looping background video underneath a playing bumper is work no
  // one can see, and a church laptop feels it.
  const fullScreen = coversScreen(state);
  const media = state.kind === "blackout" || fullScreen ? null : mediaBackground(theme);

  // A stable signature of the *content* so the slide fades in only on real slide
  // changes — not on every countdown tick.
  const contentKey =
    state.kind === "verse" || state.kind === "song"
      ? `${state.kind}:${state.caption}:${state.text}`
      : state.kind === "image"
        ? `image:${state.src.slice(0, 64)}`
        : state.kind === "video"
        ? `video:${state.src}`
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
      {/* Text sizes this wrapper by its own content. Full-screen media does not:
          an absolutely-positioned child contributes no size, so the wrapper
          collapsed to zero and `inset-0` inside it resolved to nothing at all,
          which is why a projected image or video came out blank while verses
          were fine. Media gets an explicitly full-screen wrapper instead.
          Inline positioning, because two Tailwind position utilities on one
          element are settled by stylesheet order rather than by class order. */}
      <div
        key={contentKey}
        className="proj-fade z-10 flex flex-col items-center justify-center"
        style={fullScreen ? { position: "absolute", inset: 0 } : { position: "relative" }}
      >
        {body()}
      </div>

      {/* Sound under the service. Rendered whatever is on the wall, and keyed by
          source so a new track loads while a nudge of the volume does not
          restart the one already playing. Nothing to see: it is audio. */}
      {audio.src !== "" && (
        <audio
          key={audio.src}
          ref={audioRef}
          src={assetSrc(audio.src)}
          loop={audio.looping}
          autoPlay={!audio.paused}
        />
      )}

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
