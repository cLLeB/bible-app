import { describe, expect, it } from "vitest";
import type { Theme } from "../api";
import { backgroundCss, bodyRem, bodyStyle, captionStyle } from "./theme";

const solid: Theme = {
  id: "t",
  name: "T",
  background: { kind: "color", color: "#010203", color2: "#000000", angle: 0 },
  text: {
    fontFamily: "Inter, sans-serif",
    color: "#ffffff",
    captionColor: "#cccccc",
    align: "center",
    weight: 700,
    shadow: true,
    uppercase: true,
  },
  builtIn: false,
};

const gradient: Theme = {
  ...solid,
  background: { kind: "gradient", color: "#111111", color2: "#222222", angle: 160 },
  text: { ...solid.text, shadow: false, uppercase: false, weight: 400 },
};

describe("backgroundCss", () => {
  it("returns the solid colour for a color background", () => {
    expect(backgroundCss(solid)).toBe("#010203");
  });
  it("builds a linear-gradient for a gradient background", () => {
    expect(backgroundCss(gradient)).toBe("linear-gradient(160deg, #111111, #222222)");
  });
});

describe("bodyRem auto-fit", () => {
  it("shrinks as the passage grows", () => {
    const short = bodyRem(50, 1);
    const long = bodyRem(600, 1);
    expect(short).toBeGreaterThan(long);
    expect(short).toBe(3.2);
    expect(long).toBeCloseTo(1.4);
  });
  it("multiplies by the global font scale", () => {
    expect(bodyRem(50, 2)).toBeCloseTo(6.4);
  });
  it("treats a 0/undefined scale as 1", () => {
    expect(bodyRem(50, 0)).toBe(3.2);
  });
});

describe("bodyStyle", () => {
  it("applies weight, alignment, uppercase and legibility shadow from the theme", () => {
    const s = bodyStyle(solid, 50, 1);
    expect(s.fontWeight).toBe(700);
    expect(s.textAlign).toBe("center");
    expect(s.textTransform).toBe("uppercase");
    expect(s.color).toBe("#ffffff");
    expect(s.textShadow).not.toBe("none");
    expect(s.fontSize).toBe("3.2rem");
  });
  it("drops the shadow and uppercase when the theme disables them", () => {
    const s = bodyStyle(gradient, 50, 1);
    expect(s.textShadow).toBe("none");
    expect(s.textTransform).toBe("none");
  });
});

describe("captionStyle", () => {
  it("uses the caption colour and scales with fontScale", () => {
    expect(captionStyle(solid, 1).color).toBe("#cccccc");
    expect(captionStyle(solid, 2).fontSize).toBe(`${1.4 * 2}rem`);
  });
});
