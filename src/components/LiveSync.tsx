import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { VersePayload } from "../api";
import { mirrorsToConsole } from "../lib/presenting";
import { useLiveStore, useScriptureStore } from "../services";
import { pushScriptureStage } from "../stage";

/**
 * Backend-driven presentation, mirrored into the console's own state. Renders
 * nothing, and is mounted once outside the tab switch.
 *
 * That placement is the whole point. The phone remote and the listening loop
 * keep presenting while the operator is on Prepare, and the stage monitor has to
 * follow them there too. While this lived inside the scripture presenter, which
 * only mounts on Live, the first verse sent from a phone reached the wall and
 * the stage learned nothing: the component holding the listener was on the tab
 * the operator had just left to set the phone up.
 */
export function LiveSync() {
  const current = useScriptureStore((s) => s.current);

  useEffect(() => {
    const sub = listen<{ verse: VersePayload; source: string }>("presenting-changed", (e) => {
      if (!mirrorsToConsole(e.payload.source)) return;
      useLiveStore.getState().setOwner("scripture");
      useScriptureStore.getState().setCurrent(e.payload.verse);
    });
    return () => {
      sub.then((f) => f());
    };
  }, []);

  // Mirror the live verse, and the one after it, onto the stage monitor. Only
  // while scripture owns the screen: during a service the run order drives the
  // stage, so "next" there is the next cue rather than the next verse.
  useEffect(() => {
    if (current && useLiveStore.getState().owner === "scripture") {
      void pushScriptureStage(current);
    }
  }, [current]);

  return null;
}
