import { present } from "../present";
import { useScriptureStore } from "../services";

export function RecentVerses() {
  const recents = useScriptureStore((s) => s.recents);
  if (recents.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <span className="panel-title">Recent</span>
      {recents.map((v, i) => (
        <button
          key={`${v.reference}-${v.translation}-${i}`}
          onClick={() => present(v)}
          className="btn btn-sm"
          title={v.text}
        >
          {v.reference}
        </button>
      ))}
    </div>
  );
}
