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
