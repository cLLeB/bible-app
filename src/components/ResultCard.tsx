import { useEffect, useState } from "react";
import {
  blankProjection,
  listTranslations,
  projectParallel,
  type TranslationInfo,
} from "../api";
import { present } from "../present";
import { useServiceStore } from "../services";
import { useLookupStore } from "../store";

export function ResultCard() {
  const { result, error } = useLookupStore();
  const addVerse = useServiceStore((s) => s.addVerse);
  const [translations, setTranslations] = useState<TranslationInfo[]>([]);
  const [secondary, setSecondary] = useState("");

  useEffect(() => {
    listTranslations().then(setTranslations).catch(() => {});
  }, []);

  if (error) return <p className="text-red-600">{error}</p>;
  if (!result) return null;

  // Offer every installed translation except the one already showing.
  const others = translations.filter((t) => t.code !== result.translation);
  const secondaryCode = secondary || others[0]?.code || "";

  return (
    <div className="rounded-xl border p-4" style={{ background: "var(--surface-2)" }}>
      <div className="mb-1 text-sm text-gray-500">
        {result.reference} · {result.translation}
      </div>
      <p className="mb-3 text-lg">{result.text}</p>
      <div className="flex flex-wrap items-center gap-2">
        <button onClick={() => present(result)} className="btn btn-primary">
          Project
        </button>
        <button onClick={() => blankProjection()} className="btn">
          Blank
        </button>
        <button onClick={() => addVerse(result)} className="btn">
          ＋ Service
        </button>
        {others.length > 0 && (
          <>
            <span className="ml-1 text-sm text-[var(--muted)]">Compare with</span>
            <select
              className="select"
              style={{ width: "auto" }}
              value={secondaryCode}
              onChange={(e) => setSecondary(e.target.value)}
            >
              {others.map((t) => (
                <option key={t.code} value={t.code}>
                  {t.code}
                </option>
              ))}
            </select>
            <button
              onClick={() =>
                projectParallel(result.bookOsis, result.chapter, result.verse, secondaryCode)
              }
              className="btn"
              title="Show this verse in both translations side by side"
            >
              Both
            </button>
          </>
        )}
      </div>
    </div>
  );
}
