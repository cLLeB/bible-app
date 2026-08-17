import { useEffect, useState } from "react";
import { pjlinkCommand } from "../api";

// PJLink Class-1 command bodies.
const CMD = {
  powerOn: "%1POWR 1",
  powerOff: "%1POWR 0",
  blankOn: "%1AVMT 31",
  blankOff: "%1AVMT 30",
  powerQuery: "%1POWR ?",
} as const;

interface Config {
  host: string;
  port: number;
  password: string;
}

const STORE_KEY = "pjlink-projector";

function loadConfig(): Config {
  try {
    const raw = localStorage.getItem(STORE_KEY);
    if (raw) return JSON.parse(raw) as Config;
  } catch {
    /* ignore */
  }
  return { host: "", port: 4352, password: "" };
}

/** Human-readable read of a `%1POWR=` response. */
function powerLabel(resp: string): string {
  if (resp.includes("POWR=0")) return "Off / standby";
  if (resp.includes("POWR=1")) return "On";
  if (resp.includes("POWR=2")) return "Cooling";
  if (resp.includes("POWR=3")) return "Warming up";
  return resp;
}

export function ProjectorPanel() {
  const [cfg, setCfg] = useState<Config>(loadConfig);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    localStorage.setItem(STORE_KEY, JSON.stringify(cfg));
  }, [cfg]);

  async function run(body: string, describe?: (resp: string) => string): Promise<void> {
    if (!cfg.host.trim()) {
      setStatus("Enter the projector's IP address first.");
      return;
    }
    setBusy(true);
    try {
      const resp = await pjlinkCommand(cfg.host.trim(), cfg.port, cfg.password, body);
      setStatus(describe ? describe(resp) : resp === "%1POWR=OK" || resp.endsWith("=OK") ? "OK" : resp);
    } catch (e) {
      setStatus(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="space-y-3">
      <h2 className="panel-title">Projector (PJLink)</h2>

      <div className="flex flex-wrap items-center gap-2 text-sm">
        <input
          value={cfg.host}
          onChange={(e) => setCfg({ ...cfg, host: e.target.value })}
          placeholder="Projector IP (e.g. 192.168.1.50)"
          className="input w-52"
        />
        <input
          type="number"
          value={cfg.port}
          onChange={(e) => setCfg({ ...cfg, port: Number(e.target.value) || 4352 })}
          className="input w-20 text-center"
        />
        <input
          type="password"
          value={cfg.password}
          onChange={(e) => setCfg({ ...cfg, password: e.target.value })}
          placeholder="Password (if set)"
          className="input w-40"
        />
      </div>

      <div className="flex flex-wrap gap-2">
        <button className="btn" disabled={busy} onClick={() => run(CMD.powerOn)}>
          Power on
        </button>
        <button className="btn" disabled={busy} onClick={() => run(CMD.powerOff)}>
          Power off
        </button>
        <button className="btn" disabled={busy} onClick={() => run(CMD.blankOn)}>
          Blank
        </button>
        <button className="btn" disabled={busy} onClick={() => run(CMD.blankOff)}>
          Unblank
        </button>
        <button className="btn" disabled={busy} onClick={() => run(CMD.powerQuery, powerLabel)}>
          Status
        </button>
      </div>

      {status && <p className="text-sm text-[var(--muted)]">{busy ? "…" : status}</p>}
    </section>
  );
}
