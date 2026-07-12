import { blankProjection, projectVerse } from "../api";
import { useServiceStore } from "../services";
import { useLookupStore } from "../store";

export function ResultCard() {
  const { result, error } = useLookupStore();
  const addVerse = useServiceStore((s) => s.addVerse);
  if (error) return <p className="text-red-600">{error}</p>;
  if (!result) return null;
  return (
    <div className="rounded border p-4">
      <div className="mb-1 text-sm text-gray-500">
        {result.reference} · {result.translation}
      </div>
      <p className="mb-3 text-lg">{result.text}</p>
      <div className="flex gap-2">
        <button
          onClick={() => projectVerse(result)}
          className="rounded bg-green-600 px-4 py-2 text-white"
        >
          Project
        </button>
        <button onClick={() => blankProjection()} className="rounded border px-4 py-2">
          Blank
        </button>
        <button onClick={() => addVerse(result)} className="rounded border px-4 py-2">
          ＋ Service
        </button>
      </div>
    </div>
  );
}
