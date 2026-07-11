import { FormEvent, useEffect, useState } from "react";
import { addSong, updateSong, type SongDetail } from "../api";
import { groupEveryNLines, splitLyrics } from "../lib/slides";

interface SongEditorProps {
  editing: SongDetail | null;
  onSaved: () => void;
  onCancel: () => void;
}

export function SongEditor({ editing, onSaved, onCancel }: SongEditorProps) {
  const [title, setTitle] = useState("");
  const [author, setAuthor] = useState("");
  const [lyrics, setLyrics] = useState("");
  const [linesPerSlide, setLinesPerSlide] = useState(4);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setTitle(editing?.title ?? "");
    setAuthor(editing?.author ?? "");
    setLyrics(editing?.lyrics ?? "");
    setError(null);
  }, [editing]);

  const preview = splitLyrics(lyrics);

  async function onSubmit(e: FormEvent): Promise<void> {
    e.preventDefault();
    if (!title.trim() || !lyrics.trim()) return;
    try {
      if (editing) {
        await updateSong(editing.id, title.trim(), author.trim() || null, lyrics);
      } else {
        await addSong(title.trim(), author.trim() || null, lyrics);
      }
      setTitle("");
      setAuthor("");
      setLyrics("");
      onSaved();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <form onSubmit={onSubmit} className="space-y-2 rounded border p-3">
      <div className="flex items-center justify-between">
        <h3 className="font-semibold">{editing ? `Edit: ${editing.title}` : "Add song"}</h3>
        {editing && (
          <button type="button" onClick={onCancel} className="text-sm text-gray-500 underline">
            cancel
          </button>
        )}
      </div>
      {error && <p className="text-red-600">{error}</p>}
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

      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-2">
          <textarea
            value={lyrics}
            onChange={(e) => setLyrics(e.target.value)}
            placeholder={"Paste lyrics here. A blank line starts a new slide."}
            rows={10}
            className="w-full rounded border px-2 py-1 font-mono text-sm"
          />
          <div className="flex items-center gap-2 text-sm">
            <button
              type="button"
              onClick={() => setLyrics(groupEveryNLines(lyrics, linesPerSlide))}
              className="rounded border px-2 py-1"
            >
              Auto-format
            </button>
            <span>every</span>
            <input
              type="number"
              min={1}
              max={12}
              value={linesPerSlide}
              onChange={(e) => setLinesPerSlide(Math.max(1, Number(e.target.value) || 1))}
              className="w-14 rounded border px-1 py-1"
            />
            <span>lines/slide</span>
          </div>
        </div>

        <div className="space-y-1">
          <div className="text-xs text-gray-500">Preview — {preview.length} slide(s)</div>
          <div className="max-h-64 space-y-1 overflow-y-auto">
            {preview.map((slide, i) => (
              <div key={i} className="rounded border bg-gray-50 p-2">
                <div className="text-[10px] text-gray-400">Slide {i + 1}</div>
                <p className="whitespace-pre-line text-sm">{slide}</p>
              </div>
            ))}
          </div>
        </div>
      </div>

      <button type="submit" className="rounded bg-blue-600 px-3 py-1 text-white">
        {editing ? "Save changes" : "Add song"}
      </button>
    </form>
  );
}
