import { FormEvent } from "react";
import { lookupReference } from "../api";
import { present } from "../present";
import { usePreviewStore } from "../services";
import { useLookupStore } from "../store";

export function LookupBar() {
  const { query, setQuery, setResult, setError } = useLookupStore();

  // Enter projects immediately (fastest path). Shift+Enter stages it in the
  // preview pane instead, which is what "preview" meant here all along: the
  // result card showed the words, the pane shows what the wall will look like.
  async function submit(project: boolean): Promise<void> {
    try {
      const v = await lookupReference(query.trim());
      setResult(v);
      if (project) {
        await present(v);
        setQuery("");
      } else {
        usePreviewStore
          .getState()
          .stage({ kind: "verse", text: v.text, caption: `${v.reference} · ${v.translation}` });
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
        className="input flex-1"
        style={{ height: "2.75rem", fontSize: "1.05rem" }}
      />
      <button type="submit" className="btn btn-lg btn-primary">
        Project
      </button>
    </form>
  );
}
