import { useRef, useState } from "react";
import * as pdfjsLib from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { blankProjection, projectImage } from "../api";

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl;

// Render pages wide enough to look crisp on a projector without exploding memory.
const RENDER_WIDTH = 1600;

async function renderPdf(bytes: ArrayBuffer, onProgress: (done: number, total: number) => void): Promise<string[]> {
  const pdf = await pdfjsLib.getDocument({ data: new Uint8Array(bytes) }).promise;
  const pages: string[] = [];
  for (let i = 1; i <= pdf.numPages; i++) {
    const page = await pdf.getPage(i);
    const base = page.getViewport({ scale: 1 });
    const scale = RENDER_WIDTH / base.width;
    const viewport = page.getViewport({ scale });
    const canvas = document.createElement("canvas");
    canvas.width = viewport.width;
    canvas.height = viewport.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("canvas unavailable");
    await page.render({ canvas, canvasContext: ctx, viewport }).promise;
    pages.push(canvas.toDataURL("image/jpeg", 0.85));
    onProgress(i, pdf.numPages);
  }
  return pages;
}

export function PdfImport() {
  const [pages, setPages] = useState<string[]>([]);
  const [live, setLive] = useState(-1);
  const [status, setStatus] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  async function onFile(file: File): Promise<void> {
    setStatus("Rendering…");
    setPages([]);
    setLive(-1);
    try {
      const bytes = await file.arrayBuffer();
      const rendered = await renderPdf(bytes, (d, t) => setStatus(`Rendering ${d}/${t}…`));
      setPages(rendered);
      setStatus(`${rendered.length} page${rendered.length === 1 ? "" : "s"} ready`);
    } catch (e) {
      setStatus(e instanceof Error ? e.message : String(e));
    }
  }

  function project(i: number): void {
    if (i < 0 || i >= pages.length) return;
    setLive(i);
    void projectImage(pages[i]);
  }

  return (
    <section className="space-y-2">
      <div className="flex items-center justify-between">
        <h2 className="panel-title">Slides / PDF</h2>
        <div className="flex gap-2">
          <button className="btn btn-sm" onClick={() => inputRef.current?.click()}>
            Import PDF
          </button>
          {pages.length > 0 && (
            <button
              className="btn btn-sm"
              onClick={() => {
                setLive(-1);
                void blankProjection();
              }}
            >
              Blank
            </button>
          )}
        </div>
      </div>
      <p className="text-xs text-[var(--faint)]">
        Export a PowerPoint/Keynote deck to PDF, then import it here. Click a page to project it.
      </p>
      <input
        ref={inputRef}
        type="file"
        accept="application/pdf,.pdf"
        className="hidden"
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) void onFile(f);
          e.target.value = "";
        }}
      />

      {status && <p className="text-sm text-[var(--muted)]">{status}</p>}

      {pages.length > 0 && (
        <div className="grid grid-cols-3 gap-2 sm:grid-cols-4">
          {pages.map((src, i) => (
            <button
              key={i}
              onClick={() => project(i)}
              className="overflow-hidden rounded border"
              style={{ borderColor: i === live ? "var(--accent, #3b82f6)" : "var(--border)", borderWidth: i === live ? 2 : 1 }}
              title={`Project page ${i + 1}`}
            >
              <img src={src} alt={`Page ${i + 1}`} className="block h-auto w-full" />
            </button>
          ))}
        </div>
      )}
    </section>
  );
}
