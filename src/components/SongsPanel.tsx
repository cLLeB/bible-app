import { useEffect, useState } from "react";
import {
  deleteSong,
  exportSongs,
  getSong,
  importSongs,
  listSongs,
  type SongDetail,
  type SongSummary,
} from "../api";
import { useServiceStore } from "../services";
import { SongEditor } from "./SongEditor";
import { SongLive } from "./SongLive";

export function SongsPanel() {
  const [songs, setSongs] = useState<SongSummary[]>([]);
  const [selected, setSelected] = useState<SongSummary | null>(null);
  const [editing, setEditing] = useState<SongDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [backup, setBackup] = useState<string | null>(null);
  const [importText, setImportText] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const addSongCue = useServiceStore((s) => s.addSong);

  async function onExport(): Promise<void> {
    const json = await exportSongs();
    setBackup(json);
    try {
      await navigator.clipboard.writeText(json);
      setNotice("Songs copied to clipboard — paste into a file to save.");
    } catch {
      setNotice("Copy the text below to save your songs.");
    }
  }

  async function onImport(): Promise<void> {
    try {
      const count = await importSongs(importText);
      setImportText("");
      setNotice(`Imported ${count} song(s).`);
      await refresh();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  const shown = songs.filter((s) =>
    `${s.title} ${s.author ?? ""}`.toLowerCase().includes(filter.toLowerCase()),
  );

  async function refresh(): Promise<void> {
    try {
      setSongs(await listSongs());
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function onEdit(song: SongSummary): Promise<void> {
    try {
      setEditing(await getSong(song.id));
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onDelete(song: SongSummary): Promise<void> {
    try {
      await deleteSong(song.id);
      if (selected?.id === song.id) setSelected(null);
      if (editing?.id === song.id) setEditing(null);
      await refresh();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <section className="space-y-4">
      <h2 className="text-xl font-semibold">Songs</h2>
      {error && <p className="text-red-600">{error}</p>}

      <SongEditor
        editing={editing}
        onSaved={() => {
          setEditing(null);
          void refresh();
        }}
        onCancel={() => setEditing(null)}
      />

      <div className="grid grid-cols-2 gap-4">
        <div className="space-y-2">
          <input
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Filter songs…"
            className="w-full rounded border px-2 py-1 text-sm"
          />
          <ul className="space-y-1">
            {shown
              .filter((s) => !s.builtIn)
              .map((s) => (
                <li key={s.id} className="flex items-center gap-1">
                  <button
                    onClick={() => setSelected(s)}
                    className={`flex-1 rounded px-2 py-1 text-left ${
                      selected?.id === s.id ? "bg-gray-200" : "hover:bg-gray-100"
                    }`}
                  >
                    {s.title}
                    {s.author ? <span className="text-gray-500"> — {s.author}</span> : null}
                  </button>
                  <button
                    onClick={() => addSongCue(s.id, s.title)}
                    className="rounded border px-2 py-1 text-xs"
                    title="Add to service order"
                  >
                    ＋
                  </button>
                  <button onClick={() => onEdit(s)} className="rounded border px-2 py-1 text-xs">
                    Edit
                  </button>
                  <button
                    onClick={() => onDelete(s)}
                    className="rounded border px-2 py-1 text-xs text-red-600"
                  >
                    Del
                  </button>
                </li>
              ))}
            {shown.filter((s) => !s.builtIn).length === 0 && (
              <li className="text-sm text-gray-500">No songs of yours yet.</li>
            )}
          </ul>

          {shown.some((s) => s.builtIn) && (
            <details className="rounded border p-2 text-sm">
              <summary className="cursor-pointer text-gray-600">
                Bundled hymns ({shown.filter((s) => s.builtIn).length}) — read-only
              </summary>
              <ul className="mt-2 max-h-72 space-y-1 overflow-y-auto pr-1">
                {shown
                  .filter((s) => s.builtIn)
                  .map((s) => (
                    <li key={s.id} className="flex items-center gap-1">
                      <button
                        onClick={() => setSelected(s)}
                        className={`flex-1 rounded px-2 py-1 text-left ${
                          selected?.id === s.id ? "bg-gray-200" : "hover:bg-gray-100"
                        }`}
                      >
                        {s.title}
                        {s.author ? <span className="text-gray-500"> — {s.author}</span> : null}
                      </button>
                      <button
                        onClick={() => addSongCue(s.id, s.title)}
                        className="rounded border px-2 py-1 text-xs"
                        title="Add to service order"
                      >
                        ＋
                      </button>
                    </li>
                  ))}
              </ul>
            </details>
          )}
        </div>

        <div>{selected && <SongLive song={selected} />}</div>
      </div>

      <details className="rounded border p-2 text-sm">
        <summary className="cursor-pointer text-gray-600">Backup / share songs</summary>
        <div className="mt-2 space-y-2">
          {notice && <p className="text-green-700">{notice}</p>}
          <button onClick={onExport} className="rounded border px-3 py-1">
            Export all songs (copy)
          </button>
          {backup && (
            <textarea
              readOnly
              value={backup}
              rows={4}
              className="w-full rounded border px-2 py-1 font-mono text-xs"
            />
          )}
          <textarea
            value={importText}
            onChange={(e) => setImportText(e.target.value)}
            placeholder="Paste exported songs JSON here to import…"
            rows={3}
            className="w-full rounded border px-2 py-1 font-mono text-xs"
          />
          <button
            onClick={onImport}
            disabled={!importText.trim()}
            className="rounded bg-blue-600 px-3 py-1 text-white disabled:opacity-50"
          >
            Import songs
          </button>
        </div>
      </details>
    </section>
  );
}
