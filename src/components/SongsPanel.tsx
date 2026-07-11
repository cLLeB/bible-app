import { useEffect, useState } from "react";
import { deleteSong, getSong, listSongs, type SongDetail, type SongSummary } from "../api";
import { SongEditor } from "./SongEditor";
import { SongLive } from "./SongLive";

export function SongsPanel() {
  const [songs, setSongs] = useState<SongSummary[]>([]);
  const [selected, setSelected] = useState<SongSummary | null>(null);
  const [editing, setEditing] = useState<SongDetail | null>(null);
  const [error, setError] = useState<string | null>(null);

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
        <ul className="space-y-1">
          {songs.map((s) => (
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
          {songs.length === 0 && <li className="text-sm text-gray-500">No songs yet.</li>}
        </ul>

        <div>{selected && <SongLive song={selected} />}</div>
      </div>
    </section>
  );
}
