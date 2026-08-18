import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getTranslation, listTranslations, setTranslation, type TranslationInfo } from "../api";

export function TranslationPicker() {
  const [translations, setTranslations] = useState<TranslationInfo[]>([]);
  const [active, setActive] = useState("");

  useEffect(() => {
    async function load(): Promise<void> {
      const list = await listTranslations();
      setTranslations(list);
      // Keyed by version on purpose. The console remembers the operator's choice,
      // which is right, but it also meant a machine that had ever opened WEB would
      // keep opening WEB no matter what the shipped default became - the remembered
      // value is read first and then pushed back to the backend. Bumping the key
      // retires the old answer once, so the new default applies, and every choice
      // made after that is remembered as before.
      const saved = localStorage.getItem("translation.v2");
      const current = saved && list.some((t) => t.code === saved) ? saved : await getTranslation();
      setActive(current);
      if (saved && saved !== (await getTranslation())) await setTranslation(saved);
    }
    void load();

    // A spoken/typed "...in ASV" switches translation on the backend — reflect it.
    const sub = listen<string>("translation-changed", (e) => {
      setActive(e.payload);
      localStorage.setItem("translation.v2", e.payload);
    });
    // A newly downloaded translation should appear in the picker immediately.
    const installed = listen<string>("translation-installed", () => {
      void listTranslations().then(setTranslations);
    });
    return () => {
      sub.then((f) => f());
      installed.then((f) => f());
    };
  }, []);

  async function onChange(code: string): Promise<void> {
    setActive(code);
    localStorage.setItem("translation.v2", code);
    await setTranslation(code);
  }

  if (translations.length <= 1) return null; // nothing to switch

  return (
    <select
      value={active}
      onChange={(e) => onChange(e.target.value)}
      className="select h-9 text-sm"
      style={{ width: "auto", maxWidth: "16rem" }}
      title="Active translation"
    >
      {translations.map((t) => (
        <option key={t.code} value={t.code}>
          {t.code} · {t.name}
        </option>
      ))}
    </select>
  );
}
