import type { CSSProperties } from "react";
import type { Theme } from "../api";

/**
 * Pure theme → CSS translation for the projection screen. Kept free of React so
 * the sizing and styling rules can be unit-tested without a DOM. Both the live
 * projection window and the operator's theme preview render through these, so
 * what the operator previews is exactly what the congregation sees.
 */

/** The CSS `background` value for a theme (solid colour or linear gradient). */
export function backgroundCss(theme: Theme): string {
  const bg = theme.background;
  if (bg.kind === "gradient") {
    return `linear-gradient(${bg.angle}deg, ${bg.color}, ${bg.color2})`;
  }
  return bg.color;
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
    fontSize: `${1.4 * (fontScale || 1)}rem`,
  };
}
