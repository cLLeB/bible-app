import { useEffect, useState } from "react";
import { getTranslation, listTranslations, setTranslation, type TranslationInfo } from "../api";

export function TranslationPicker() {
  const [translations, setTranslations] = useState<TranslationInfo[]>([]);
  const [active, setActive] = useState("");

  useEffect(() => {
    async function load(): Promise<void> {
      const list = await listTranslations();
      setTranslations(list);
      const saved = localStorage.getItem("translation");
      const current = saved && list.some((t) => t.code === saved) ? saved : await getTranslation();
      setActive(current);
      if (saved && saved !== (await getTranslation())) await setTranslation(saved);
    }
    void load();
  }, []);

  async function onChange(code: string): Promise<void> {
    setActive(code);
    localStorage.setItem("translation", code);
    await setTranslation(code);
  }

  if (translations.length <= 1) return null; // nothing to switch

  return (
    <label className="flex items-center gap-2 text-sm">
      Translation
      <select
        value={active}
        onChange={(e) => onChange(e.target.value)}
        className="rounded border px-2 py-1"
      >
        {translations.map((t) => (
          <option key={t.code} value={t.code}>
            {t.code} — {t.name}
          </option>
        ))}
      </select>
    </label>
  );
}
