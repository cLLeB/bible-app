import { useEffect, useState } from "react";
import {
  downloadTranslation,
  translationCatalog,
  type CatalogEntry,
} from "../api";

/// Lets the operator add more (free / public-domain) translations for offline
/// use. Downloading needs internet once; the text is then stored locally.
export function TranslationManager() {
  const [open, setOpen] = useState(false);
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function load(): Promise<void> {
    try {
      setCatalog(await translationCatalog());
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  useEffect(() => {
    if (open) void load();
  }, [open]);

  async function onDownload(code: string): Promise<void> {
    setError(null);
    setBusy(code);
    try {
      await downloadTranslation(code);
      await load();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="relative text-sm">
      <button onClick={() => setOpen((v) => !v)} className="btn btn-sm">
        {open ? "Close" : "＋ Translations"}
      </button>

      {open && (
        <div
          className="absolute right-0 z-30 mt-2 w-80 rounded-xl border p-2 shadow-lg"
          style={{ background: "var(--surface)", borderColor: "var(--border)" }}
        >
          <p className="mb-2 text-xs text-[var(--muted)]">
            Free & public-domain translations. Downloading needs internet once, then
            works fully offline.
          </p>
          {error && <p className="tint tint-bad mb-2 rounded p-1 text-xs">{error}</p>}
          <ul className="space-y-1">
            {catalog.map((t) => (
              <li key={t.code} className="flex items-center justify-between gap-2">
                <span className="truncate" title={t.name}>
                  <span className="font-semibold">{t.code}</span>{" "}
                  <span className="text-[var(--muted)]">{t.name}</span>
                  {t.licensed && (
                    <span
                      className="tint-strong tint-warn ml-1 rounded px-1 text-[10px]"
                      title="Copyrighted — for your personal use only, not for distribution"
                    >
                      personal
                    </span>
                  )}
                </span>
                {t.installed ? (
                  <span className="shrink-0 text-xs text-green-700">✓ installed</span>
                ) : (
                  <button
                    onClick={() => onDownload(t.code)}
                    disabled={busy !== null}
                    className="btn btn-sm btn-primary shrink-0"
                  >
                    {busy === t.code ? "Downloading…" : "Download"}
                  </button>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
