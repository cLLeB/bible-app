import { useEffect, useState } from "react";
import { blankProjection, clearAlert, setProjection, showStage } from "../api";
import { grouped, isShortcut, lookup } from "../lib/hotkeys";

/**
 * The service-wide shortcuts, and the `?` sheet that says what they are.
 *
 * Renders nothing until asked. It sits outside the Live/Prepare switch so the keys
 * work on both tabs: an operator reaching for Blank does not first check which tab
 * they left open.
 *
 * Scripture navigation is deliberately not handled here. ScripturePresenter already
 * owns the arrow keys and only acts when it holds the live cursor, which is the
 * behaviour that should win; the arrows appear in the sheet because the operator
 * needs to know they exist, not because this component implements them.
 */
export function Hotkeys() {
  const [showSheet, setShowSheet] = useState(false);

  useEffect(() => {
    function onKey(e: KeyboardEvent): void {
      if (!isShortcut(e)) return;
      const hit = lookup(e.key);
      if (!hit) return;

      switch (hit.key) {
        case "b":
          e.preventDefault();
          void blankProjection().catch(() => undefined);
          break;
        case "k":
          e.preventDefault();
          void setProjection({ kind: "blackout" }).catch(() => undefined);
          break;
        case "l":
          e.preventDefault();
          void setProjection({ kind: "logo" }).catch(() => undefined);
          break;
        case "s":
          e.preventDefault();
          void showStage().catch(() => undefined);
          break;
        case "Escape":
          // Also the way out of the sheet, so it never traps anyone.
          if (showSheet) {
            setShowSheet(false);
          } else {
            void clearAlert().catch(() => undefined);
          }
          break;
        case "?":
          e.preventDefault();
          setShowSheet((v) => !v);
          break;
        default:
          break; // the arrows belong to ScripturePresenter
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [showSheet]);

  if (!showSheet) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0,0,0,0.6)" }}
      onClick={() => setShowSheet(false)}
    >
      <div
        className="max-h-[80vh] overflow-y-auto rounded-lg border p-4"
        style={{ background: "var(--surface)", borderColor: "var(--border)", minWidth: "22rem" }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-2 flex items-center gap-2">
          <h2 className="panel-title">Keyboard</h2>
          <button className="btn btn-sm ml-auto" onClick={() => setShowSheet(false)}>
            Close
          </button>
        </div>
        {grouped().map(([group, keys]) => (
          <div key={group} className="mb-3">
            <div className="text-xs uppercase tracking-wide text-[var(--faint)]">{group}</div>
            <table className="mt-1 w-full text-sm">
              <tbody>
                {keys.map((k) => (
                  <tr key={k.key}>
                    <td className="py-0.5 pr-4">
                      <kbd
                        className="rounded border px-1.5 py-0.5 text-xs"
                        style={{ borderColor: "var(--border)" }}
                      >
                        {k.key === " " ? "Space" : k.key}
                      </kbd>
                    </td>
                    <td className="py-0.5 text-[var(--muted)]">{k.label}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ))}
      </div>
    </div>
  );
}
