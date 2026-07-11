import { FormEvent, useEffect, useState } from "react";
import {
  addSong,
  getSongSlides,
  listSongs,
  projectSlide,
  type Slide,
  type SongSummary,
} from "../api";

export function SongsPanel() {
  const [songs, setSongs] = useState<SongSummary[]>([]);
  const [title, setTitle] = useState("");
  const [author, setAuthor] = useState("");
  const [lyrics, setLyrics] = useState("");
  const [selected, setSelected] = useState<SongSummary | null>(null);
  const [slides, setSlides] = useState<Slide[]>([]);
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

  async function onAdd(e: FormEvent): Promise<void> {
    e.preventDefault();
    if (!title.trim() || !lyrics.trim()) return;
    try {
      await addSong(title.trim(), author.trim() || null, lyrics);
      setTitle("");
      setAuthor("");
      setLyrics("");
      await refresh();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function onSelect(song: SongSummary): Promise<void> {
    setSelected(song);
    try {
      setSlides(await getSongSlides(song.id));
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <section className="space-y-4">
      <h2 className="text-xl font-semibold">Songs</h2>
      {error && <p className="text-red-600">{error}</p>}

      <form onSubmit={onAdd} className="space-y-2 rounded border p-3">
        <div className="flex gap-2">
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Title"
            className="flex-1 rounded border px-2 py-1"
          />
          <input
            value={author}
            onChange={(e) => setAuthor(e.target.value)}
            placeholder="Author (optional)"
            className="flex-1 rounded border px-2 py-1"
          />
        </div>
        <textarea
          value={lyrics}
          onChange={(e) => setLyrics(e.target.value)}
          placeholder={"Lyrics — separate slides with a blank line"}
          rows={5}
          className="w-full rounded border px-2 py-1 font-mono text-sm"
        />
        <button type="submit" className="rounded bg-blue-600 px-3 py-1 text-white">
          Add song
        </button>
      </form>

      <div className="grid grid-cols-2 gap-4">
        <ul className="space-y-1">
          {songs.map((s) => (
            <li key={s.id}>
              <button
                onClick={() => onSelect(s)}
                className={`w-full rounded px-2 py-1 text-left ${
                  selected?.id === s.id ? "bg-gray-200" : "hover:bg-gray-100"
                }`}
              >
                {s.title}
                {s.author ? <span className="text-gray-500"> — {s.author}</span> : null}
              </button>
            </li>
          ))}
          {songs.length === 0 && <li className="text-sm text-gray-500">No songs yet.</li>}
        </ul>

        <div className="space-y-2">
          {selected &&
            slides.map((slide) => (
              <div key={slide.orderIndex} className="rounded border p-2">
                <div className="mb-1 text-xs text-gray-500">Slide {slide.orderIndex + 1}</div>
                <p className="mb-2 whitespace-pre-line text-sm">{slide.text}</p>
                <button
                  onClick={() => projectSlide(selected.id, slide.orderIndex)}
                  className="rounded bg-green-600 px-3 py-1 text-sm text-white"
                >
                  Project
                </button>
              </div>
            ))}
        </div>
      </div>
    </section>
  );
}
