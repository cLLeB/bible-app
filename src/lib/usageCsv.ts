import type { UsageRow } from "../api";

/** Escape a value for CSV (quote if it contains a comma, quote, or newline). */
function cell(value: string): string {
  if (/[",\n]/.test(value)) return `"${value.replace(/"/g, '""')}"`;
  return value;
}

/**
 * Render the song-usage report as CSV for CCLI reporting. Pure so it's testable
 * and reusable (download, clipboard). Header row + one row per song.
 */
export function usageCsv(rows: UsageRow[]): string {
  const header = ["Title", "Author", "Times used", "Last used"];
  const lines = [header.join(",")];
  for (const r of rows) {
    lines.push(
      [cell(r.title), cell(r.author ?? ""), String(r.times), cell(r.lastUsed ?? "")].join(","),
    );
  }
  return lines.join("\n");
}
