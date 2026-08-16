/**
 * Media rules shared by the library, the slideshow and the projection window.
 *
 * Kept free of React and of Tauri so the parts that decide *what happens* can be
 * tested without a window or a backend. The same extension list exists in
 * `src-tauri/src/media.rs`; the two are checked against each other by a test
 * there, because a file the console offers but the backend refuses is the kind
 * of split that only shows up mid-service.
 */

export type MediaKind = "image" | "video" | "audio";

export interface MediaItem {
  id: number;
  path: string;
  title: string;
  kind: MediaKind;
}

const IMAGE_EXT = ["jpg", "jpeg", "png", "gif", "webp", "bmp", "avif"];
const VIDEO_EXT = ["mp4", "m4v", "mov", "webm", "mkv", "avi"];
// Sound-only files. They sit in the same library as the pictures because an
// operator thinks of "the things I brought for Sunday" as one pile, but they
// never take the screen.
const AUDIO_EXT = ["mp3", "m4a", "wav", "ogg", "oga", "flac", "aac", "opus"];

/** Everything the file picker should offer, by kind. */
export const MEDIA_EXTENSIONS: Readonly<Record<MediaKind, readonly string[]>> = {
  image: IMAGE_EXT,
  video: VIDEO_EXT,
  audio: AUDIO_EXT,
};

/** What kind of media a path is, or null when it is not media we can show. */
export function mediaKind(path: string): MediaKind | null {
  const dot = path.lastIndexOf(".");
  if (dot < 0) return null;
  const ext = path.slice(dot + 1).toLowerCase();
  if (IMAGE_EXT.includes(ext)) return "image";
  if (VIDEO_EXT.includes(ext)) return "video";
  if (AUDIO_EXT.includes(ext)) return "audio";
  return null;
}

/** The file's own name, without directories or extension, as a starting title. */
export function titleFromPath(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path;
  const dot = name.lastIndexOf(".");
  return (dot > 0 ? name.slice(0, dot) : name).trim() || name;
}

/** Does this kind take the congregation screen? Audio does not. */
export function isVisual(kind: MediaKind): boolean {
  return kind !== "audio";
}

/** Seconds a slideshow holds each item. */
export const SLIDESHOW_MIN_SECONDS = 2;
export const SLIDESHOW_MAX_SECONDS = 600;
export const SLIDESHOW_DEFAULT_SECONDS = 8;

/**
 * A usable dwell time from whatever the operator typed. An empty or nonsense
 * box must not stop a service, so it falls back rather than throwing, and the
 * floor keeps a mistyped 0 from spinning the projector through the whole
 * library in a second.
 */
export function clampInterval(value: number | string): number {
  const n = typeof value === "string" ? Number(value.trim()) : value;
  if (!Number.isFinite(n) || n <= 0) return SLIDESHOW_DEFAULT_SECONDS;
  return Math.min(SLIDESHOW_MAX_SECONDS, Math.max(SLIDESHOW_MIN_SECONDS, Math.round(n)));
}

/**
 * The next slide's index, or null when the run is over. Looping never ends, so
 * an announcements loop can be left running before a service starts.
 */
export function nextIndex(current: number, count: number, looping: boolean): number | null {
  if (count <= 0) return null;
  const next = current + 1;
  if (next < count) return next;
  return looping ? 0 : null;
}
