import { useEffect, useState } from "react";
import { listSongs, pcoImportPlan, type SongSummary } from "../api";
import { useServiceStore } from "../services";

interface Config {
  appId: string;
  secret: string;
  serviceTypeId: string;
  planId: string;
}

const STORE_KEY = "planning-center";

function loadConfig(): Config {
  try {
    const raw = localStorage.getItem(STORE_KEY);
    if (raw) return JSON.parse(raw) as Config;
  } catch {
    /* ignore */
  }
  return { appId: "", secret: "", serviceTypeId: "", planId: "" };
}

interface Result {
  added: string[];
  notFound: string[];
  skipped: number;
}

export function PlanningCenterPanel() {
  const [cfg, setCfg] = useState<Config>(loadConfig);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<Result | null>(null);
  const addSongCue = useServiceStore((s) => s.addSong);

  useEffect(() => {
    localStorage.setItem(STORE_KEY, JSON.stringify(cfg));
  }, [cfg]);

  async function importPlan(): Promise<void> {
    setError(null);
    setResult(null);
    if (!cfg.appId || !cfg.secret || !cfg.serviceTypeId || !cfg.planId) {
      setError("Fill in all four fields.");
      return;
    }
    setBusy(true);
    try {
      const [items, songs] = await Promise.all([
        pcoImportPlan(cfg.appId, cfg.secret, cfg.serviceTypeId, cfg.planId),
        listSongs(),
      ]);
      const byTitle = new Map<string, SongSummary>();
      for (const s of songs) byTitle.set(s.title.trim().toLowerCase(), s);

      const added: string[] = [];
      const notFound: string[] = [];
      let skipped = 0;
      for (const item of items) {
        const match = byTitle.get(item.title.trim().toLowerCase());
        if (match) {
          addSongCue(match.id, match.title);
          added.push(match.title);
        } else if (item.kind === "song") {
          notFound.push(item.title);
        } else {
          skipped += 1;
        }
      }
      setResult({ added, notFound, skipped });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="space-y-2">
      <h2 className="panel-title">Planning Center import</h2>

      <div className="grid grid-cols-2 gap-2 text-sm">
        <input
          value={cfg.appId}
          onChange={(e) => setCfg({ ...cfg, appId: e.target.value })}
          placeholder="Application ID"
          className="input"
        />
        <input
          type="password"
          value={cfg.secret}
          onChange={(e) => setCfg({ ...cfg, secret: e.target.value })}
          placeholder="Secret"
          className="input"
        />
        <input
          value={cfg.serviceTypeId}
          onChange={(e) => setCfg({ ...cfg, serviceTypeId: e.target.value })}
          placeholder="Service Type ID"
          className="input"
        />
        <input
          value={cfg.planId}
          onChange={(e) => setCfg({ ...cfg, planId: e.target.value })}
          placeholder="Plan ID"
          className="input"
        />
      </div>

      <button className="btn btn-primary btn-sm" disabled={busy} onClick={importPlan}>
        {busy ? "Importing…" : "Import plan"}
      </button>

      {error && <p className="tint tint-bad rounded px-2 py-1 text-sm">{error}</p>}

      {result && (
        <div className="space-y-1 text-sm">
          <p className="text-[var(--muted)]">
            Added {result.added.length} song{result.added.length === 1 ? "" : "s"} to the service
            order{result.skipped > 0 ? ` · ${result.skipped} non-song item(s) skipped` : ""}.
          </p>
          {result.notFound.length > 0 && (
            <div>
              <p className="text-[var(--faint)]">Not in your library yet (add them in Songs):</p>
              <ul className="list-inside list-disc text-[var(--muted)]">
                {result.notFound.map((t, i) => (
                  <li key={i}>{t}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </section>
  );
}
