import { FormEvent } from "react";
import { lookupReference } from "../api";
import { present } from "../present";
import { useLookupStore } from "../store";

export function LookupBar() {
  const { query, setQuery, setResult, setError } = useLookupStore();

  // Enter projects immediately (fastest path). Shift+Enter just previews.
  async function submit(project: boolean): Promise<void> {
    try {
      const v = await lookupReference(query.trim());
      setResult(v);
      if (project) {
        await present(v);
        setQuery("");
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  function onSubmit(e: FormEvent): void {
    e.preventDefault();
    void submit(true);
  }

  return (
    <form onSubmit={onSubmit} className="flex gap-2">
      <input
        autoFocus
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && e.shiftKey) {
            e.preventDefault();
            void submit(false);
          }
        }}
        placeholder="e.g. John 3:16  (Enter projects · Shift+Enter previews)"
        className="flex-1 rounded border px-3 py-2 text-lg"
      />
      <button type="submit" className="rounded bg-green-600 px-4 py-2 text-white">
        Project
      </button>
    </form>
  );
}
