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
      // The backend is the only place this is kept. The console used to remember it
      // too, in localStorage, and the two disagreed: a machine that had once opened
      // WEB kept opening WEB however the shipped default changed.
      setActive(await getTranslation());
    }

    void load();

    // A spoken/typed "...in ASV" switches translation on the backend — reflect it.
    const sub = listen<string>("translation-changed", (e) => {
      setActive(e.payload);
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
