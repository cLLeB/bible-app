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
import { parseSongsText, songsToText, type SongText } from "../lib/songs";
import { SongEditor } from "./SongEditor";
import { SongLive } from "./SongLive";

export function SongsPanel() {
  const [songs, setSongs] = useState<SongSummary[]>([]);
  const [selected, setSelected] = useState<SongSummary | null>(null);
  const [editing, setEditing] = useState<SongDetail | null>(null);
  const [showEditor, setShowEditor] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [backup, setBackup] = useState<string | null>(null);
  const [importText, setImportText] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const addSongCue = useServiceStore((s) => s.addSong);

  async function onExport(): Promise<void> {
    const json = await exportSongs();
    const text = songsToText(JSON.parse(json) as SongText[]);
    setBackup(text);
    try {
      await navigator.clipboard.writeText(text);
      setNotice("Songs copied to clipboard. Paste into a document to save.");
    } catch {
      setNotice("Copy the text below to save your songs.");
    }
  }

  async function onImport(): Promise<void> {
    try {
      const songs = parseSongsText(importText);
      if (songs.length === 0) {
        setError("No songs found. Put a title on its own line, then the lyrics.");
        return;
      }
      const count = await importSongs(JSON.stringify(songs));
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
      setShowEditor(true);
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

  const userSongs = shown.filter((s) => !s.builtIn);
  const bundled = shown.filter((s) => s.builtIn);

  const songRow = (s: SongSummary, readOnly: boolean) => (
    <li key={s.id} className="flex items-center gap-1">
      <button
        onClick={() => setSelected(s)}
        className={`flex-1 truncate rounded-md px-2 py-1.5 text-left text-sm ${
          selected?.id === s.id ? "tint tint-current" : "tint-neutral tint-hover"
        }`}
      >
        {s.title}
        {s.author ? <span className="text-[var(--muted)]"> · {s.author}</span> : null}
      </button>
      <button onClick={() => addSongCue(s.id, s.title)} className="icon-btn" title="Add to service order">
        ＋
      </button>
      {!readOnly && (
        <>
          <button onClick={() => onEdit(s)} className="btn btn-sm">Edit</button>
          <button onClick={() => onDelete(s)} className="btn btn-sm btn-danger">Del</button>
        </>
      )}
    </li>
  );

  return (
    <section className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="panel-title">Songs</h2>
        <input
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="Filter…"
          className="input ml-auto h-8 w-40 text-sm"
        />
        <button
          onClick={() => {
            setEditing(null);
            setShowEditor((v) => !v);
          }}
          className="btn btn-sm btn-primary"
        >
          {showEditor ? "Close" : "＋ New song"}
        </button>
      </div>
      {error && <p className="text-sm text-red-600">{error}</p>}

      {(showEditor || editing) && (
        <SongEditor
          editing={editing}
          onSaved={() => {
            setEditing(null);
            setShowEditor(false);
            void refresh();
          }}
          onCancel={() => {
            setEditing(null);
            setShowEditor(false);
          }}
        />
      )}

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <div className="space-y-2">
          {userSongs.length > 0 ? (
            <ul className="space-y-0.5">{userSongs.map((s) => songRow(s, false))}</ul>
          ) : (
            <p className="text-sm text-gray-400">No songs of yours yet. Add one with “＋ New song”.</p>
          )}

          {bundled.length > 0 && (
            <details className="rounded-lg border p-2">
              <summary className="cursor-pointer text-sm text-[var(--muted)]">
                Hymns ({bundled.length})
              </summary>
              <ul className="mt-2 max-h-80 space-y-0.5 overflow-y-auto pr-1">
                {bundled.map((s) => songRow(s, true))}
              </ul>
            </details>
          )}
        </div>

        <div>{selected && <SongLive song={selected} />}</div>
      </div>

      <details className="rounded-lg border p-2">
        <summary className="cursor-pointer text-sm text-[var(--muted)]">Backup / share songs</summary>
        <div className="mt-2 space-y-2">
          {notice && <p className="text-sm text-green-700">{notice}</p>}
          <button onClick={onExport} className="btn btn-sm">Copy my songs as text</button>
          {backup && (
            <textarea readOnly value={backup} rows={6} className="textarea w-full text-sm" />
          )}
          <p className="text-xs text-[var(--muted)]">
            To add songs, paste them below: one per block, a title on its own line, an
            optional “by Author” line, then the lyrics. Separate multiple songs with a line of “===”.
          </p>
          <textarea
            value={importText}
            onChange={(e) => setImportText(e.target.value)}
            placeholder={"Amazing Grace\nby John Newton\n\nAmazing grace, how sweet the sound…"}
            rows={5}
            className="textarea w-full text-sm"
          />
          <button onClick={onImport} disabled={!importText.trim()} className="btn btn-sm btn-primary">
            Import songs
          </button>
        </div>
      </details>
    </section>
  );
}
