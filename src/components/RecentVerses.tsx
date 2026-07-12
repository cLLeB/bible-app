import { present } from "../present";
import { useScriptureStore } from "../services";

export function RecentVerses() {
  const recents = useScriptureStore((s) => s.recents);
  if (recents.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-1">
      <span className="text-xs uppercase text-gray-400">Recent</span>
      {recents.map((v, i) => (
        <button
          key={`${v.reference}-${v.translation}-${i}`}
          onClick={() => present(v)}
          className="rounded border px-2 py-0.5 text-xs hover:bg-gray-100"
          title={v.text}
        >
          {v.reference}
        </button>
      ))}
    </div>
  );
}
