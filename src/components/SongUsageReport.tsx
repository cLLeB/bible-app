import { useState } from "react";
import { songUsageReport, type UsageRow } from "../api";
import { usageCsv } from "../lib/usageCsv";

export function SongUsageReport() {
  const [rows, setRows] = useState<UsageRow[] | null>(null);
  const [copied, setCopied] = useState(false);

  function load(): void {
    songUsageReport().then(setRows).catch(() => setRows([]));
  }

  async function copyCsv(): Promise<void> {
    if (!rows) return;
    try {
      await navigator.clipboard.writeText(usageCsv(rows));
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      /* clipboard unavailable — the table is still on screen to read from */
    }
  }

  return (
    <section className="space-y-2">
      <div className="flex items-center justify-between">
        <h2 className="panel-title">Song usage (CCLI)</h2>
        <div className="flex gap-2">
          <button className="btn btn-sm" onClick={load}>
            {rows ? "Refresh" : "Show report"}
          </button>
          {rows && rows.length > 0 && (
            <button className="btn btn-sm" onClick={copyCsv}>
              {copied ? "Copied ✓" : "Copy CSV"}
            </button>
          )}
        </div>
      </div>

      {rows && rows.length === 0 && (
        <p className="text-sm text-[var(--faint)]">
          No songs projected yet — usage is recorded once per song per day.
        </p>
      )}

      {rows && rows.length > 0 && (
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm">
            <thead className="text-xs uppercase text-[var(--faint)]">
              <tr>
                <th className="py-1 pr-3">Title</th>
                <th className="py-1 pr-3">Author</th>
                <th className="py-1 pr-3 text-right">Times</th>
                <th className="py-1">Last used</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r, i) => (
                <tr key={i} className="border-t" style={{ borderColor: "var(--border)" }}>
                  <td className="py-1 pr-3">{r.title}</td>
                  <td className="py-1 pr-3 text-[var(--muted)]">{r.author ?? "—"}</td>
                  <td className="py-1 pr-3 text-right tabular-nums">{r.times}</td>
                  <td className="py-1 text-[var(--muted)]">{r.lastUsed ?? "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
