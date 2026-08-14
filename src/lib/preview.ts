import type { ProjectionState } from "../api";
import { coversScreen } from "./projection";

/** What a staged item reads as: the big text, and the caption under it. */
export interface PreviewLines {
  body: string;
  caption: string;
  /** True when the item is a picture rather than words, so the pane shows the
   *  file itself instead of a text block. */
  visual: boolean;
}

/**
 * Reduce any projection state to what the preview pane should show.
 *
 * Kept pure and separate from the pane so every content type is covered on
 * purpose rather than by whichever branch someone remembered to add. The pane
 * is the operator's promise about what the congregation is about to see; a kind
 * that silently previews as blank is a broken promise.
 */
export function previewLines(state: ProjectionState): PreviewLines {
  // `visual` is the same question the projection window asks when it decides
  // how to size its content wrapper, so the two share one answer.
  const visual = coversScreen(state);
  switch (state.kind) {
    case "verse":
    case "song":
      return { body: state.text, caption: state.caption, visual };
    case "parallel":
      return {
        body: `${state.primaryText}\n\n${state.secondaryText}`,
        caption: `${state.caption} (${state.primaryCode} / ${state.secondaryCode})`,
        visual,
      };
    case "message":
      return { body: state.text, caption: "", visual };
    case "countdown":
      return { body: "0:00", caption: state.label, visual };
    case "image":
      return { body: "", caption: "Image", visual };
    case "video":
      return { body: "", caption: `Video · ${state.title}`, visual };
    case "logo":
      return { body: "✝", caption: "Logo", visual };
    case "blackout":
      return { body: "", caption: "Blackout", visual };
    case "blank":
    default:
      return { body: "", caption: "Blank", visual };
  }
}

/** A short label for the staged item, for buttons and headings. */
export function previewLabel(state: ProjectionState): string {
  const { caption, body } = previewLines(state);
  if (caption) return caption;
  const firstLine = body.split("\n")[0]?.trim() ?? "";
  return firstLine.length > 40 ? `${firstLine.slice(0, 40)}…` : firstLine || "Nothing";
}
