import { FormEvent } from "react";
import { lookupReference } from "../api";
import { useLookupStore } from "../store";

export function LookupBar() {
  const { query, setQuery, setResult, setError } = useLookupStore();

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    try {
      const v = await lookupReference(query.trim());
      setResult(v);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <form onSubmit={onSubmit} className="flex gap-2">
      <input
        autoFocus
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="e.g. John 3:16"
        className="flex-1 rounded border px-3 py-2 text-lg"
      />
      <button type="submit" className="rounded bg-blue-600 px-4 py-2 text-white">
        Look up
      </button>
    </form>
  );
}
