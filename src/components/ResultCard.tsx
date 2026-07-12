import { blankProjection } from "../api";
import { present } from "../present";
import { useServiceStore } from "../services";
import { useLookupStore } from "../store";

export function ResultCard() {
  const { result, error } = useLookupStore();
  const addVerse = useServiceStore((s) => s.addVerse);
  if (error) return <p className="text-red-600">{error}</p>;
  if (!result) return null;
  return (
    <div className="rounded-xl border p-4" style={{ background: "var(--surface-2)" }}>
      <div className="mb-1 text-sm text-gray-500">
        {result.reference} · {result.translation}
      </div>
      <p className="mb-3 text-lg">{result.text}</p>
      <div className="flex flex-wrap gap-2">
        <button onClick={() => present(result)} className="btn btn-primary">
          Project
        </button>
        <button onClick={() => blankProjection()} className="btn">
          Blank
        </button>
        <button onClick={() => addVerse(result)} className="btn">
          ＋ Service
        </button>
      </div>
    </div>
  );
}
