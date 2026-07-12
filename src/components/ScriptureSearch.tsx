import { FormEvent, useState } from "react";
import { searchScripture, type VersePayload } from "../api";
import { present } from "../present";
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
          className="input flex-1"
        />
        <button type="submit" className="btn">
          Search
        </button>
      </form>
      {searched && results.length === 0 && (
        <p className="text-sm text-gray-400">No matches.</p>
      )}
      {results.length > 0 && (
        <div className="max-h-64 space-y-1 overflow-y-auto">
          {results.map((v, i) => (
            <div key={`${v.reference}-${i}`} className="rounded-lg border p-2.5">
              <div className="mb-1 text-sm font-semibold">{v.reference}</div>
              <p className="mb-2 text-xs text-gray-600">{v.text}</p>
              <div className="flex gap-2">
                <button onClick={() => present(v)} className="btn btn-sm btn-primary">
                  Project
                </button>
                <button onClick={() => addVerse(v)} className="btn btn-sm">
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
