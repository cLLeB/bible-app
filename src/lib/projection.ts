import type { ProjectionState } from "../api";

/**
 * Does this state fill the whole screen with a picture rather than words?
 *
 * Three places need the same answer and must not drift apart: the projection
 * window sizes its content wrapper by it, skips decoding a theme background
 * behind it, and the preview pane shows the file itself instead of a text
 * block. Getting it wrong is not subtle — an absolutely-positioned child gives
 * a content-sized wrapper no height, so the item projects as nothing at all.
 */
export function coversScreen(state: ProjectionState): boolean {
  return state.kind === "image" || state.kind === "video";
}

/** Sources a webview can already load, and must not be rewritten. */
const READY = /^(data:|blob:|https?:|asset:|tauri:|\/)/i;

/**
 * Does this source have to be turned into an asset URL before a webview can
 * load it?
 *
 * Media used to arrive as data URLs, which load anywhere, and now arrives as
 * absolute file paths, which load nowhere until converted. The projection
 * window missed that change while the preview pane did not, so a deck page
 * previewed perfectly and projected as a blank screen. One rule, asked by both.
 */
export function needsAssetUrl(src: string): boolean {
  return src.trim() !== "" && !READY.test(src);
}
