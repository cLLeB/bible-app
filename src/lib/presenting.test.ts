import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { mirrorsToConsole } from "./presenting";

describe("mirrorsToConsole", () => {
  it("applies what the console did not cause", () => {
    expect(mirrorsToConsole("remote")).toBe(true);
    expect(mirrorsToConsole("voice")).toBe(true);
  });

  it("ignores the console's own changes", () => {
    // Re-applying one would take keyboard ownership away from a service order
    // mid-run, because the service projects its verse cues through the same
    // backend path.
    expect(mirrorsToConsole("console")).toBe(false);
  });
});

describe("LiveSync placement", () => {
  const app = readFileSync(new URL("../App.tsx", import.meta.url), "utf8");

  it("is mounted outside the tab switch", () => {
    // The bug this guards: the phone remote is set up from Prepare, but the
    // listener that mirrors it onto the stage monitor lived in a component that
    // only mounts on Live. The first verse sent from a phone reached the wall
    // and the stage never heard about it.
    const mount = app.indexOf("<LiveSync");
    // The branch that swaps the whole layout, not the nav button's own styling.
    const tabSwitch = app.indexOf('{tab === "live" ? (');
    expect(mount, "LiveSync is not mounted in App").toBeGreaterThan(-1);
    expect(tabSwitch, "the tab switch moved; this test needs updating").toBeGreaterThan(-1);
    expect(mount).toBeLessThan(tabSwitch);
  });

  it("keeps the backend mirror out of the tab-local presenter", () => {
    const presenter = readFileSync(
      new URL("../components/ScripturePresenter.tsx", import.meta.url),
      "utf8",
    );
    expect(presenter).not.toContain("presenting-changed");
    expect(presenter).not.toContain("pushScriptureStage");
  });
});
