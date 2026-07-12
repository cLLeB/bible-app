import { FormEvent, useState } from "react";
import { projectVerse, searchScripture, type VersePayload } from "../api";
import { useServiceStore } from "../services";

export function ScriptureSearch() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<VersePayload[]>([]);
  const [searched, setSearched] = useState(false);
  const addVerse = useServiceStore((s) => s.addVerse);

  async function onSubmit(e: FormEvent): Promise<void> {
    e.preventDefault();
    if (!query.trim()) return;
    setResults(await searchScripture(query.trim()));
    setSearched(true);
  }

  return (
    <div className="space-y-2">
      <form onSubmit={onSubmit} className="flex gap-2">
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search scripture by word, e.g. shepherd"
          className="flex-1 rounded border px-3 py-2"
        />
        <button type="submit" className="rounded border px-4 py-2">
          Search
        </button>
      </form>
      {searched && results.length === 0 && (
        <p className="text-sm text-gray-400">No matches.</p>
      )}
      {results.length > 0 && (
        <div className="max-h-64 space-y-1 overflow-y-auto">
          {results.map((v, i) => (
            <div key={`${v.reference}-${i}`} className="rounded border p-2">
              <div className="mb-1 text-sm font-semibold">{v.reference}</div>
              <p className="mb-2 text-xs text-gray-600">{v.text}</p>
              <div className="flex gap-2">
                <button
                  onClick={() => projectVerse(v)}
                  className="rounded bg-green-600 px-2 py-1 text-xs text-white"
                >
                  Project
                </button>
                <button onClick={() => addVerse(v)} className="rounded border px-2 py-1 text-xs">
                  ＋ Service
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
