import { useCallback, useEffect, useState } from "react";
import QRCode from "qrcode";
import { listen } from "@tauri-apps/api/event";
import {
  listDisplays,
  setOutputDisplay,
  showStage,
  startRemote,
  type DisplayInfo,
} from "../api";

/** Printed size of the QR, in CSS pixels. */
const QR_PX = 190;

/**
 * Where the projection goes: the stage monitor, a phone remote, and browser /
 * NDI output. All of it is set up once before a service, which is why it lives
 * on Prepare rather than beside the mid-service controls.
 */
export function OutputsPanel() {
  const [urls, setUrls] = useState<string[]>([]);
  const [picked, setPicked] = useState<string | null>(null);
  const [qr, setQr] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [displays, setDisplays] = useState<DisplayInfo[]>([]);
  const [chosenDisplay, setChosenDisplay] = useState("");

  const loadDisplays = useCallback(async (): Promise<void> => {
    try {
      const [found, chosen] = await listDisplays();
      setDisplays(found);
      setChosenDisplay(chosen);
    } catch {
      /* the picker is an aid; the automatic choice still works without it */
    }
  }, []);

  useEffect(() => {
    void loadDisplays();
    // The app watches for screens arriving and leaving, so this list follows a
    // TV being plugged in without anyone pressing Refresh.
    const sub = listen<DisplayInfo[]>("displays-changed", (e) => setDisplays(e.payload));
    return () => {
      sub.then((f) => f());
    };
  }, [loadDisplays]);

  async function pickDisplay(name: string): Promise<void> {
    setChosenDisplay(name);
    setError(null);
    try {
      await setOutputDisplay(name);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  // Scanning the QR just opens the address. The typed address stays on screen
  // underneath: a camera is not always to hand, and a QR that fails to render
  // must not strand anyone.
  useEffect(() => {
    if (!picked) {
      setQr(null);
      return;
    }
    let cancelled = false;
    QRCode.toDataURL(picked, {
      margin: 2,
      width: QR_PX * 2, // rendered at 2x so it stays sharp on high-DPI screens
      errorCorrectionLevel: "M",
      // Fixed black on white regardless of app theme. A themed QR scans badly,
      // and the pale quiet zone around it is part of the spec.
      color: { dark: "#000000", light: "#ffffff" },
    })
      .then((url) => {
        if (!cancelled) setQr(url);
      })
      .catch(() => {
        if (!cancelled) setQr(null);
      });
    return () => {
      cancelled = true;
    };
  }, [picked]);

  async function begin(): Promise<void> {
    setError(null);
    try {
      const found = await startRemote();
      setUrls(found);
      // Keep the operator's choice across a refresh if it is still valid,
      // otherwise fall back to the best guess.
      setPicked((current) => (current && found.includes(current) ? current : (found[0] ?? null)));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function copy(value: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      /* clipboard unavailable; the value is on screen to read from */
    }
  }

  return (
    <section className="space-y-3">
      <h2 className="panel-title">Outputs</h2>

      {/* Which screen the congregation sees. Named rather than guessed, because
          "the second monitor" is an enumeration order, not a place. */}
      <div className="rounded border p-2 text-sm" style={{ borderColor: "var(--border)" }}>
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs uppercase tracking-wide text-[var(--faint)]">
            Congregation screen
          </span>
          <select
            className="select ml-auto"
            style={{ width: "auto" }}
            value={chosenDisplay}
            onChange={(e) => void pickDisplay(e.target.value)}
          >
            <option value="">Automatic (the screen you are not using)</option>
            {displays.map((d) => (
              <option key={d.name} value={d.name}>
                {d.name} · {d.width}×{d.height}
                {d.primary ? " (this laptop)" : ""}
              </option>
            ))}
          </select>
          <button onClick={() => void loadDisplays()} className="btn btn-sm" title="Re-check after plugging in a TV">
            Refresh
          </button>
        </div>
        <p className="mt-1 text-xs text-[var(--faint)]">
          {displays.length <= 1
            ? "Only this laptop's screen is connected, so output opens as a window instead of covering the console. Plug in a TV and press Refresh."
            : "Output fills that screen and never takes keyboard focus, so you can keep typing here while it is live."}
        </p>
      </div>

      <div className="flex flex-wrap gap-2">
        <button onClick={() => showStage()} className="btn">
          Stage display
        </button>
        <button onClick={begin} className="btn">
          {urls.length > 0 ? "Refresh addresses" : "Phone remote"}
        </button>
      </div>

      {error && <p className="tint tint-bad rounded px-2 py-1 text-sm">{error}</p>}

      {picked && (
        <div className="space-y-3 text-sm">
          <div className="rounded border p-3" style={{ borderColor: "var(--border)" }}>
            <div className="text-xs uppercase tracking-wide text-[var(--faint)]">Phone remote</div>

            <div className="mt-2 flex flex-wrap items-start gap-4">
              {qr && (
                <div className="flex-none text-center">
                  <img
                    src={qr}
                    alt={`Scan to open the phone remote at ${picked}`}
                    width={QR_PX}
                    height={QR_PX}
                    style={{ borderRadius: "6px", display: "block", background: "#fff" }}
                  />
                  <div className="mt-1 text-xs text-[var(--faint)]">Scan with the phone camera</div>
                </div>
              )}

              <div className="min-w-[16rem] flex-1">
                <p className="text-[var(--muted)]">Or open this on the phone:</p>
                <div className="mt-1 flex flex-wrap items-center gap-2">
                  <span className="font-mono text-base">{picked}</span>
                  <button className="btn btn-sm" onClick={() => copy(picked)}>
                    {copied ? "Copied ✓" : "Copy"}
                  </button>
                </div>

                {urls.length > 1 && (
                  <div className="mt-3">
                    <p className="text-xs text-[var(--faint)]">
                      This laptop is on more than one network. If the phone can&apos;t reach that
                      address, pick the one matching the network the phone is on (a phone hotspot is
                      usually 172.20.10.x):
                    </p>
                    <div className="mt-1 flex flex-wrap gap-1">
                      {urls.map((u) => (
                        <button
                          key={u}
                          onClick={() => setPicked(u)}
                          aria-pressed={u === picked}
                          className={`btn btn-sm font-mono ${u === picked ? "btn-primary" : ""}`}
                        >
                          {u.replace("http://", "").replace(":8787", "")}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>

          <div>
            <div className="text-xs uppercase tracking-wide text-[var(--faint)]">
              OBS / browser output
            </div>
            <p className="text-[var(--muted)]">
              Add a Browser Source at{" "}
              <span className="font-mono text-[var(--text)]">{picked}/projection</span>.
            </p>
          </div>

          {/* Folded away: NDI is a once-per-venue setup, and most services never
              touch it. The heading stays visible so it can still be found. */}
          <details>
            <summary className="cursor-pointer select-none text-xs uppercase tracking-wide text-[var(--faint)]">
              NDI (vMix / OBS / Resolume)
            </summary>
            <p className="mt-1 text-[var(--muted)]">
              Add the Browser Source above in OBS, then turn on OBS&rsquo;s NDI output (Tools → NDI
              Output Settings → Main Output). vMix and other NDI tools then see the live projection
              as a source on the network, with no extra hardware.
            </p>
          </details>
        </div>
      )}
    </section>
  );
}
