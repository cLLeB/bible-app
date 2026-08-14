import type { ProjectionState } from "../api";

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
  switch (state.kind) {
    case "verse":
    case "song":
      return { body: state.text, caption: state.caption, visual: false };
    case "parallel":
      return {
        body: `${state.primaryText}\n\n${state.secondaryText}`,
        caption: `${state.caption} (${state.primaryCode} / ${state.secondaryCode})`,
        visual: false,
      };
    case "message":
      return { body: state.text, caption: "", visual: false };
    case "countdown":
      return { body: "0:00", caption: state.label, visual: false };
    case "image":
      return { body: "", caption: "Image", visual: true };
    case "video":
      return { body: "", caption: `Video · ${state.title}`, visual: true };
    case "logo":
      return { body: "✝", caption: "Logo", visual: false };
    case "blackout":
      return { body: "", caption: "Blackout", visual: false };
    case "blank":
    default:
      return { body: "", caption: "Blank", visual: false };
  }
}

/** A short label for the staged item, for buttons and headings. */
export function previewLabel(state: ProjectionState): string {
  const { caption, body } = previewLines(state);
  if (caption) return caption;
  const firstLine = body.split("\n")[0]?.trim() ?? "";
  return firstLine.length > 40 ? `${firstLine.slice(0, 40)}…` : firstLine || "Nothing";
}
