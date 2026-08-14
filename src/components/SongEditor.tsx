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

  // If the pasted lyrics have no blank-line verse separation (one big block),
  // auto-group them into readable slides so they display nicely without the
  // operator having to format anything by hand.
  const autoFormatted =
    lyrics.trim() && splitLyrics(lyrics).length <= 1
      ? groupEveryNLines(lyrics, linesPerSlide)
      : lyrics;
  const preview = splitLyrics(autoFormatted);

  async function onSubmit(e: FormEvent): Promise<void> {
    e.preventDefault();
    if (!title.trim() || !lyrics.trim()) return;
    try {
      if (editing) {
        await updateSong(editing.id, title.trim(), author.trim() || null, autoFormatted);
      } else {
        await addSong(title.trim(), author.trim() || null, autoFormatted);
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
    <form onSubmit={onSubmit} className="space-y-2.5 rounded-xl border p-3" style={{ background: "var(--surface-2)" }}>
      <div className="flex items-center justify-between">
        <h3 className="font-semibold">{editing ? `Edit: ${editing.title}` : "Add song"}</h3>
        <button type="button" onClick={onCancel} className="btn btn-sm">
          Cancel
        </button>
      </div>
      {error && <p className="text-sm text-red-600">{error}</p>}
      <div className="flex flex-col gap-2 sm:flex-row">
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Title"
          className="input flex-1"
        />
        <input
          value={author}
          onChange={(e) => setAuthor(e.target.value)}
          placeholder="Author (optional)"
          className="input flex-1"
        />
      </div>

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        <div className="space-y-2">
          <textarea
            value={lyrics}
            onChange={(e) => setLyrics(e.target.value)}
            placeholder={"Paste lyrics here. A blank line starts a new slide."}
            rows={10}
            className="textarea w-full font-mono text-sm"
          />
          <div className="flex flex-wrap items-center gap-2 text-sm">
            <button
              type="button"
              onClick={() => setLyrics(groupEveryNLines(lyrics, linesPerSlide))}
              className="btn btn-sm"
            >
              Auto-format
            </button>
            <span className="text-[var(--muted)]">every</span>
            <input
              type="number"
              min={1}
              max={12}
              value={linesPerSlide}
              onChange={(e) => setLinesPerSlide(Math.max(1, Number(e.target.value) || 1))}
              className="input h-8 w-14 text-center"
            />
            <span className="text-[var(--muted)]">lines/slide</span>
          </div>
        </div>

        <div className="space-y-1">
          <div className="panel-title">Preview · {preview.length} slide(s)</div>
          <div className="max-h-64 space-y-1 overflow-y-auto">
            {preview.map((slide, i) => (
              <div key={i} className="rounded-lg border p-2" style={{ background: "var(--surface)" }}>
                <div className="text-[10px] text-gray-400">Slide {i + 1}</div>
                <p className="whitespace-pre-line text-sm">{slide}</p>
              </div>
            ))}
          </div>
        </div>
      </div>

      <button type="submit" className="btn btn-primary">
        {editing ? "Save changes" : "Add song"}
      </button>
    </form>
  );
}
