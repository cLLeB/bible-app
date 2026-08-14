import { useRef, useState } from "react";
import * as pdfjsLib from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { importSlide } from "../api";
import { titleFromPath } from "../lib/media";

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl;

/**
 * Render at the width of the screen the pages will land on. The old 1600px
 * JPEG was being scaled *up* on a 1920 projector and softening exactly the
 * thing a deck is made of, which is text. PNG rather than JPEG for the same
 * reason: flat colour and sharp lettering are what PNG is good at, and the
 * pages are written to disk now rather than carried in memory.
 */
const RENDER_WIDTH = 1920;

/** Strip the `data:image/png;base64,` prefix the backend does not want. */
function payload(dataUrl: string): string {
  const comma = dataUrl.indexOf(",");
  return comma < 0 ? dataUrl : dataUrl.slice(comma + 1);
}

export function PdfImport() {
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  async function onFile(file: File): Promise<void> {
    setBusy(true);
    setStatus("Reading…");
    const deck = titleFromPath(file.name);
    try {
      const bytes = await file.arrayBuffer();
      const pdf = await pdfjsLib.getDocument({ data: new Uint8Array(bytes) }).promise;
      for (let i = 1; i <= pdf.numPages; i++) {
        setStatus(`Rendering page ${i} of ${pdf.numPages}…`);
        const page = await pdf.getPage(i);
        const base = page.getViewport({ scale: 1 });
        const viewport = page.getViewport({ scale: RENDER_WIDTH / base.width });
        const canvas = document.createElement("canvas");
        canvas.width = viewport.width;
        canvas.height = viewport.height;
        const ctx = canvas.getContext("2d");
        if (!ctx) throw new Error("canvas unavailable");
        await page.render({ canvas, canvasContext: ctx, viewport }).promise;
        // One page at a time: a long deck reports real progress, a failure
        // names the page that failed, and nothing holds the whole deck at once.
        await importSlide(deck, i, payload(canvas.toDataURL("image/png")));
        canvas.width = 0;
        canvas.height = 0;
      }
      setStatus(
        `${pdf.numPages} page${pdf.numPages === 1 ? "" : "s"} added to Media as "${deck}".`,
      );
    } catch (e) {
      setStatus(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="space-y-2">
      <div className="flex items-center justify-between">
        <h2 className="panel-title">Slides / PDF</h2>
        <button className="btn btn-sm" onClick={() => inputRef.current?.click()} disabled={busy}>
          {busy ? "Importing…" : "Import PDF"}
        </button>
      </div>
      <p className="text-xs text-[var(--faint)]">
        Export a PowerPoint or Keynote deck to PDF, then import it here. Pages become
        images in Media, so they preview, project, reorder, join a service order and
        run as a slideshow like anything else, and they are still there next Sunday.
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
    </section>
  );
}
