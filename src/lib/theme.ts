import type { CSSProperties } from "react";
import type { Theme } from "../api";

/**
 * Pure theme → CSS translation for the projection screen. Kept free of React so
 * the sizing and styling rules can be unit-tested without a DOM. Both the live
 * projection window and the operator's theme preview render through these, so
 * what the operator previews is exactly what the congregation sees.
 */

/** The CSS base `background` for a theme. For image/video themes this is the
 *  solid colour drawn *behind* the media (visible while it loads). */
export function backgroundCss(theme: Theme): string {
  const bg = theme.background;
  if (bg.kind === "gradient") {
    return `linear-gradient(${bg.angle}deg, ${bg.color}, ${bg.color2})`;
  }
  return bg.color;
}

/** A media layer to render behind the text, or null for colour/gradient themes
 *  (and image/video themes with no file chosen yet). The `src` is the raw file
 *  path; the caller converts it to a webview-loadable URL. */
export function mediaBackground(
  theme: Theme,
): { kind: "image" | "video"; src: string; fit: "cover" | "contain"; dim: number } | null {
  const bg = theme.background;
  if ((bg.kind === "image" || bg.kind === "video") && bg.src.trim() !== "") {
    return { kind: bg.kind, src: bg.src, fit: bg.fit, dim: bg.dim };
  }
  return null;
}

/**
 * Auto-fit: shrink the body text as the passage grows so it never overflows the
 * screen. Returns a rem size, multiplied by the operator's global `fontScale`.
 * The breakpoints mirror the pre-theme behaviour so nothing regresses.
 */
export function bodyRem(textLength: number, fontScale: number): number {
  const base =
    textLength < 120 ? 3.2 :
    textLength < 220 ? 2.6 :
    textLength < 340 ? 2.1 :
    textLength < 500 ? 1.7 :
    1.4;
  return base * (fontScale || 1);
}

const LEGIBILITY_SHADOW = "0 2px 8px rgba(0,0,0,0.55)";

/** Body-text style for a given passage length under a theme + global scale. */
export function bodyStyle(theme: Theme, textLength: number, fontScale: number): CSSProperties {
  const t = theme.text;
  return {
    fontFamily: t.fontFamily,
    color: t.color,
    fontWeight: t.weight,
    textAlign: t.align,
    textTransform: t.uppercase ? "uppercase" : "none",
    textShadow: t.shadow ? LEGIBILITY_SHADOW : "none",
    fontSize: `${bodyRem(textLength, fontScale)}rem`,
    lineHeight: 1.15,
  };
}

/** Caption (reference/title) style — smaller, in the theme's caption colour. */
export function captionStyle(theme: Theme, fontScale: number): CSSProperties {
  const t = theme.text;
  return {
    fontFamily: t.fontFamily,
    color: t.captionColor,
    textAlign: t.align,
    textShadow: t.shadow ? LEGIBILITY_SHADOW : "none",
    fontSize: `${1.4 * (fontScale || 1) * (theme.layout?.captionScale ?? 1)}rem`,
  };
}

/**
 * How the slide is arranged: where the body sits and how much room is kept clear
 * down the sides.
 *
 * Optional chaining throughout because a theme saved before layouts existed has no
 * `layout` at all, and the answer for one of those is what the app always did.
 */
export function frameStyle(theme: Theme): CSSProperties {
  const l = theme.layout;
  const vertical = l?.vertical ?? "center";
  const margin = l?.sideMargin ?? 4;
  return {
    justifyContent:
      vertical === "top" ? "flex-start" : vertical === "bottom" ? "flex-end" : "center",
    paddingLeft: `${margin}%`,
    paddingRight: `${margin}%`,
  };
}

/** Is the reference shown, and if so, above or below the words? */
export function captionPlacement(theme: Theme): "above" | "below" | "hidden" {
  return theme.layout?.captionPosition ?? "below";
}
