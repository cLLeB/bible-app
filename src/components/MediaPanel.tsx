import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  addMedia,
  blankProjection,
  getProjection,
  listMedia,
  moveMedia,
  projectMedia,
  removeMedia,
  renameMedia,
  seekVideo,
  setVideoPlayback,
  slideshowRunning,
  startSlideshow,
  stopSlideshow,
  type MediaLibraryItem,
  type ProjectionState,
} from "../api";
import { clampInterval, MEDIA_EXTENSIONS, SLIDESHOW_DEFAULT_SECONDS } from "../lib/media";

/**
 * The media library, the video transport, and the slideshow.
 *
 * The library stores paths rather than copies, so this panel is also the only
 * place a church finds out that someone tidied the media folder: an item whose
 * file has gone is marked here instead of failing on the wall.
 */
export function MediaPanel() {
  const [items, setItems] = useState<MediaLibraryItem[]>([]);
  const [live, setLive] = useState<ProjectionState>({ kind: "blank" });
  const [running, setRunning] = useState(false);
  const [seconds, setSeconds] = useState(String(SLIDESHOW_DEFAULT_SECONDS));
  const [looping, setLooping] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<number | null>(null);
  const [draftTitle, setDraftTitle] = useState("");

  useEffect(() => {
    listMedia().then(setItems).catch(() => {});
    slideshowRunning().then(setRunning).catch(() => {});
    getProjection().then(setLive).catch(() => {});
    const subs = [
      listen<ProjectionState>("set-projection", (e) => setLive(e.payload)),
      // The slideshow runs in the backend, so it can start, advance and finish
      // without this panel being open. It says so rather than being asked.
      listen<boolean>("slideshow-changed", (e) => setRunning(e.payload)),
    ];
    return () => {
      subs.forEach((u) => u.then((f) => f()));
    };
  }, []);

  async function guard(work: () => Promise<unknown>): Promise<void> {
    setError(null);
    try {
      await work();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function pickFiles(): Promise<void> {
    const picked = await open({
      multiple: true,
      filters: [
        { name: "Images and video", extensions: [...MEDIA_EXTENSIONS.image, ...MEDIA_EXTENSIONS.video] },
      ],
    });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    await guard(async () => setItems(await addMedia(paths)));
  }

  async function commitRename(id: number): Promise<void> {
    const title = draftTitle.trim();
    setEditing(null);
    if (!title) return;
    await guard(async () => setItems(await renameMedia(id, title)));
  }

  const video = live.kind === "video" ? live : null;

  return (
    <section className="space-y-3">
      <h2 className="panel-title">Media</h2>

      <div className="flex flex-wrap items-center gap-2">
        <button onClick={pickFiles} className="btn btn-primary">
          Add files
        </button>
        <button onClick={() => void guard(() => blankProjection())} className="btn btn-sm">
          Blank
        </button>
        <span className="ml-auto text-xs text-[var(--faint)]">
          {items.length === 0 ? "Nothing yet" : `${items.length} item${items.length === 1 ? "" : "s"}`}
        </span>
      </div>

      {error && <p className="tint tint-bad rounded px-2 py-1 text-sm">{error}</p>}

      {video && (
        <div className="rounded border p-2" style={{ borderColor: "var(--border)" }}>
          <div className="text-xs uppercase tracking-wide text-[var(--faint)]">
            Playing · {video.title}
          </div>
          <div className="mt-2 flex flex-wrap items-center gap-2">
            <button
              onClick={() =>
                void guard(() => setVideoPlayback(!video.paused, video.muted, video.looping))
              }
              className="btn btn-sm"
            >
              {video.paused ? "▶ Play" : "⏸ Pause"}
            </button>
            <button onClick={() => void guard(() => seekVideo(0))} className="btn btn-sm">
              ⏮ Restart
            </button>
            <button
              onClick={() =>
                void guard(() => setVideoPlayback(video.paused, !video.muted, video.looping))
              }
              aria-pressed={video.muted}
              className={`btn btn-sm ${video.muted ? "btn-primary" : ""}`}
            >
              {video.muted ? "Muted" : "Sound on"}
            </button>
            <button
              onClick={() =>
                void guard(() => setVideoPlayback(video.paused, video.muted, !video.looping))
              }
              aria-pressed={video.looping}
              className={`btn btn-sm ${video.looping ? "btn-primary" : ""}`}
            >
              Loop
            </button>
          </div>
        </div>
      )}

      <div className="rounded border p-2" style={{ borderColor: "var(--border)" }}>
        <div className="text-xs uppercase tracking-wide text-[var(--faint)]">Slideshow</div>
        <div className="mt-2 flex flex-wrap items-center gap-2 text-sm">
          <label className="flex items-center gap-1">
            <span className="text-[var(--muted)]">Hold each</span>
            <input
              value={seconds}
              onChange={(e) => setSeconds(e.target.value)}
              inputMode="numeric"
              className="input h-9 w-16"
              aria-label="Seconds to hold each slide"
            />
            <span className="text-[var(--muted)]">s</span>
          </label>
          <label className="flex items-center gap-1">
            <input type="checkbox" checked={looping} onChange={(e) => setLooping(e.target.checked)} />
            <span className="text-[var(--muted)]">Repeat</span>
          </label>
          {running ? (
            <button onClick={() => void guard(() => stopSlideshow())} className="btn btn-sm">
              ■ Stop
            </button>
          ) : (
            <button
              onClick={() => void guard(() => startSlideshow(clampInterval(seconds), looping))}
              className="btn btn-sm btn-primary"
            >
              ▶ Start
            </button>
          )}
          {running && <span className="text-xs text-[var(--live)]">Running</span>}
        </div>
      </div>

      <ul className="space-y-1">
        {items.map((m, i) => (
          <li
            key={m.id}
            className="flex flex-wrap items-center gap-2 rounded border px-2 py-1.5 text-sm"
            style={{ borderColor: "var(--border)" }}
          >
            <span className="chip">{m.kind === "video" ? "Video" : "Image"}</span>

            {editing === m.id ? (
              <input
                value={draftTitle}
                onChange={(e) => setDraftTitle(e.target.value)}
                onBlur={() => void commitRename(m.id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void commitRename(m.id);
                  if (e.key === "Escape") setEditing(null);
                }}
                autoFocus
                className="input h-8 flex-1"
                aria-label="Title"
              />
            ) : (
              <button
                onClick={() => {
                  setEditing(m.id);
                  setDraftTitle(m.title);
                }}
                className="flex-1 truncate text-left"
                title={`${m.path}\n(click to rename)`}
              >
                {m.title}
              </button>
            )}

            {!m.present && (
              <span className="tint tint-bad rounded px-1.5 py-0.5 text-xs" title={m.path}>
                File missing
              </span>
            )}

            <button
              onClick={() => void guard(() => projectMedia(m.id))}
              disabled={!m.present}
              className="btn btn-sm"
            >
              Project
            </button>
            <button
              onClick={() => void guard(async () => setItems(await moveMedia(m.id, true)))}
              disabled={i === 0}
              className="icon-btn"
              title="Move up"
            >
              ▲
            </button>
            <button
              onClick={() => void guard(async () => setItems(await moveMedia(m.id, false)))}
              disabled={i === items.length - 1}
              className="icon-btn"
              title="Move down"
            >
              ▼
            </button>
            <button
              onClick={() => void guard(async () => setItems(await removeMedia(m.id)))}
              className="icon-btn"
              title="Remove from library (the file itself is left alone)"
            >
              ✕
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
