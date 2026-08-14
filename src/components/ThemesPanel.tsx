import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  deleteTheme,
  getProjectionSettings,
  listThemes,
  saveTheme,
  setActiveTheme,
  type Theme,
} from "../api";
import { backgroundCss, bodyStyle, captionStyle, mediaBackground } from "../lib/theme";

const FONTS: { label: string; value: string }[] = [
  { label: "Sans (Inter)", value: "Inter, 'Segoe UI', system-ui, sans-serif" },
  { label: "Serif (Georgia)", value: "Georgia, 'Times New Roman', serif" },
  { label: "System", value: "system-ui, sans-serif" },
  { label: "Mono", value: "'Cascadia Code', 'Consolas', monospace" },
];

const SAMPLE = "The Lord is my shepherd; I shall not want.";
const SAMPLE_CAPTION = "Psalm 23:1";

function newId(): string {
  return `custom-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 5)}`;
}

/** A fresh, editable custom theme seeded from an existing one. */
function duplicateAsCustom(base: Theme): Theme {
  return {
    ...base,
    id: newId(),
    name: `${base.name} copy`,
    builtIn: false,
    background: { ...base.background },
    text: { ...base.text },
  };
}

export function ThemesPanel() {
  const [themes, setThemes] = useState<Theme[]>([]);
  const [activeId, setActiveId] = useState<string>("dark");
  const [editing, setEditing] = useState<Theme | null>(null);

  function refresh(): void {
    listThemes().then(setThemes).catch(() => {});
    getProjectionSettings().then((s) => setActiveId(s.theme.id)).catch(() => {});
  }

  useEffect(refresh, []);

  function activate(id: string): void {
    setActiveId(id);
    void setActiveTheme(id).catch(() => {});
  }

  async function persist(theme: Theme, alsoActivate: boolean): Promise<void> {
    await saveTheme(theme);
    if (alsoActivate) await setActiveTheme(theme.id);
    setEditing(null);
    refresh();
  }

  async function remove(id: string): Promise<void> {
    await deleteTheme(id);
    setEditing(null);
    refresh();
  }

  // The look shown in the preview: whatever is being edited, else the active one.
  const preview = editing ?? themes.find((t) => t.id === activeId) ?? themes[0];

  return (
    <section className="space-y-3">
      <div className="flex items-center justify-between">
        <h2 className="panel-title">Themes</h2>
        {!editing && (
          <button
            className="btn"
            onClick={() => {
              const base = themes.find((t) => t.id === activeId) ?? themes[0];
              if (base) setEditing(duplicateAsCustom(base));
            }}
          >
            + New
          </button>
        )}
      </div>

      {preview && (
        <div
          className="relative flex h-32 flex-col items-center justify-center overflow-hidden rounded-lg px-6 text-center"
          style={{ background: backgroundCss(preview) }}
        >
          {(() => {
            const m = mediaBackground(preview);
            if (!m) return null;
            return (
              <>
                {m.kind === "image" ? (
                  <img src={convertFileSrc(m.src)} alt="" className="absolute inset-0 h-full w-full" style={{ objectFit: m.fit }} />
                ) : (
                  <video src={convertFileSrc(m.src)} className="absolute inset-0 h-full w-full" style={{ objectFit: m.fit }} autoPlay loop muted playsInline />
                )}
                {m.dim > 0 && <div className="absolute inset-0" style={{ background: `rgba(0,0,0,${m.dim})` }} />}
              </>
            );
          })()}
          <p className="relative z-10 line-clamp-2" style={{ ...bodyStyle(preview, SAMPLE.length, 1), fontSize: "1.25rem" }}>
            {SAMPLE}
          </p>
          <p className="relative z-10" style={{ ...captionStyle(preview, 1), fontSize: "0.85rem", marginTop: "0.4rem" }}>
            {SAMPLE_CAPTION}
          </p>
        </div>
      )}

      {!editing && (
        <div className="flex flex-wrap gap-2">
          {themes.map((t) => (
            <button
              key={t.id}
              onClick={() => activate(t.id)}
              className="flex items-center gap-2 rounded-md border px-3 py-1.5 text-sm"
              style={{
                borderColor: t.id === activeId ? "var(--accent, #3b82f6)" : "var(--border)",
                borderWidth: t.id === activeId ? 2 : 1,
              }}
              title={t.builtIn ? "Built-in: use New to make an editable copy" : "Custom theme"}
            >
              <span
                className="inline-block h-4 w-4 rounded-full border"
                style={{ background: backgroundCss(t), borderColor: "var(--border)" }}
              />
              {t.name}
              {!t.builtIn && (
                <span
                  role="button"
                  aria-label={`Edit ${t.name}`}
                  className="ml-1 text-[var(--muted)] hover:underline"
                  onClick={(e) => {
                    e.stopPropagation();
                    setEditing({ ...t, background: { ...t.background }, text: { ...t.text } });
                  }}
                >
                  ✎
                </span>
              )}
            </button>
          ))}
        </div>
      )}

      {editing && <ThemeEditor theme={editing} onChange={setEditing} onSave={persist} onDelete={remove} onCancel={() => setEditing(null)} />}
    </section>
  );
}

interface EditorProps {
  theme: Theme;
  onChange: (t: Theme) => void;
  onSave: (t: Theme, alsoActivate: boolean) => void;
  onDelete: (id: string) => void;
  onCancel: () => void;
}

function ThemeEditor({ theme, onChange, onSave, onDelete, onCancel }: EditorProps) {
  const bg = theme.background;
  const t = theme.text;
  const setBg = (patch: Partial<Theme["background"]>) => onChange({ ...theme, background: { ...bg, ...patch } });
  const setText = (patch: Partial<Theme["text"]>) => onChange({ ...theme, text: { ...t, ...patch } });

  return (
    <div className="space-y-3 rounded-lg border p-3" style={{ borderColor: "var(--border)" }}>
      <input
        className="input w-full"
        value={theme.name}
        onChange={(e) => onChange({ ...theme, name: e.target.value })}
        placeholder="Theme name"
      />

      <div className="flex flex-wrap items-center gap-3 text-sm">
        <label className="flex items-center gap-1.5">
          Background
          <select
            className="select"
            style={{ width: "auto" }}
            value={bg.kind}
            onChange={(e) => setBg({ kind: e.target.value as Theme["background"]["kind"] })}
          >
            <option value="color">Solid</option>
            <option value="gradient">Gradient</option>
            <option value="image">Image</option>
            <option value="video">Video</option>
          </select>
        </label>
        <label className="flex items-center gap-1.5" title="Background / behind-media colour">
          <input type="color" value={bg.color} onChange={(e) => setBg({ color: e.target.value })} />
        </label>
        {bg.kind === "gradient" && (
          <>
            <label className="flex items-center gap-1.5" title="Gradient end colour">
              →<input type="color" value={bg.color2} onChange={(e) => setBg({ color2: e.target.value })} />
            </label>
            <label className="flex items-center gap-1.5">
              Angle
              <input
                type="number"
                className="input w-16 text-center"
                min={0}
                max={360}
                value={bg.angle}
                onChange={(e) => setBg({ angle: Math.max(0, Math.min(360, Number(e.target.value) || 0)) })}
              />
            </label>
          </>
        )}
        {(bg.kind === "image" || bg.kind === "video") && (
          <>
            <button
              className="btn"
              onClick={async () => {
                const isVideo = bg.kind === "video";
                const picked = await open({
                  multiple: false,
                  directory: false,
                  filters: [
                    isVideo
                      ? { name: "Video", extensions: ["mp4", "webm", "mov", "mkv", "m4v"] }
                      : { name: "Image", extensions: ["jpg", "jpeg", "png", "webp", "gif", "bmp"] },
                  ],
                });
                if (typeof picked === "string") setBg({ src: picked });
              }}
            >
              {bg.src ? "Change file…" : "Choose file…"}
            </button>
            {bg.src && (
              <span className="max-w-[12rem] truncate text-xs text-[var(--muted)]" title={bg.src}>
                {bg.src.split(/[/\\]/).pop()}
              </span>
            )}
            <label className="flex items-center gap-1.5">
              Fit
              <select className="select" style={{ width: "auto" }} value={bg.fit} onChange={(e) => setBg({ fit: e.target.value as "cover" | "contain" })}>
                <option value="cover">Cover</option>
                <option value="contain">Contain</option>
              </select>
            </label>
            <label className="flex items-center gap-1.5" title="Darken the media so text stays readable">
              Dim
              <input
                type="range"
                min={0}
                max={0.8}
                step={0.05}
                value={bg.dim}
                onChange={(e) => setBg({ dim: Number(e.target.value) })}
              />
              <span className="tabular-nums text-[var(--muted)]">{Math.round(bg.dim * 100)}%</span>
            </label>
          </>
        )}
      </div>

      <div className="flex flex-wrap items-center gap-3 text-sm">
        <label className="flex items-center gap-1.5">
          Font
          <select className="select" style={{ width: "auto" }} value={t.fontFamily} onChange={(e) => setText({ fontFamily: e.target.value })}>
            {FONTS.map((f) => (
              <option key={f.value} value={f.value}>
                {f.label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-1.5" title="Text colour">
          Text<input type="color" value={t.color} onChange={(e) => setText({ color: e.target.value })} />
        </label>
        <label className="flex items-center gap-1.5" title="Caption colour">
          Caption<input type="color" value={t.captionColor} onChange={(e) => setText({ captionColor: e.target.value })} />
        </label>
      </div>

      <div className="flex flex-wrap items-center gap-3 text-sm">
        <label className="flex items-center gap-1.5">
          Align
          <select className="select" style={{ width: "auto" }} value={t.align} onChange={(e) => setText({ align: e.target.value as Theme["text"]["align"] })}>
            <option value="center">Center</option>
            <option value="left">Left</option>
            <option value="right">Right</option>
          </select>
        </label>
        <label className="flex items-center gap-1.5">
          Weight
          <select className="select" style={{ width: "auto" }} value={t.weight} onChange={(e) => setText({ weight: Number(e.target.value) })}>
            <option value={400}>Normal</option>
            <option value={600}>Semibold</option>
            <option value={700}>Bold</option>
          </select>
        </label>
        <label className="flex items-center gap-1.5">
          <input type="checkbox" checked={t.shadow} onChange={(e) => setText({ shadow: e.target.checked })} />
          Shadow
        </label>
        <label className="flex items-center gap-1.5">
          <input type="checkbox" checked={t.uppercase} onChange={(e) => setText({ uppercase: e.target.checked })} />
          Uppercase
        </label>
      </div>

      <div className="flex flex-wrap gap-2 border-t pt-3">
        <button className="btn btn-primary" onClick={() => onSave(theme, true)}>
          Save &amp; apply
        </button>
        <button className="btn" onClick={() => onSave(theme, false)}>
          Save
        </button>
        <button className="btn" onClick={onCancel}>
          Cancel
        </button>
        {!theme.builtIn && (
          <button className="btn btn-dark ml-auto" onClick={() => onDelete(theme.id)}>
            Delete
          </button>
        )}
      </div>
    </div>
  );
}
