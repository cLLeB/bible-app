import { useEffect, useState } from "react";
import * as pdfjsLib from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { deckAsPdf, importSlide, listConverters, setConverter } from "../api";
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

/** What the picker accepts. PowerPoint is converted on the way in. */
const DECK_EXTENSIONS = ["pdf", "pptx", "ppt", "ppsx", "pps", "odp"];

/** Strip the `data:image/png;base64,` prefix the backend does not want. */
function payload(dataUrl: string): string {
  const comma = dataUrl.indexOf(",");
  return comma < 0 ? dataUrl : dataUrl.slice(comma + 1);
}

export function PdfImport() {
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [converters, setConverters] = useState<[string, string][]>([]);

  useEffect(() => {
    listConverters().then(setConverters).catch(() => {});
  }, []);

  /** Let the operator name a converter this machine keeps somewhere unusual. */
  async function chooseConverter(): Promise<void> {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Converter", extensions: ["exe", ""] }],
    });
    if (!picked || Array.isArray(picked)) return;
    try {
      setConverters(await setConverter(picked));
      setStatus(null);
    } catch (e) {
      setStatus(e instanceof Error ? e.message : String(e));
    }
  }

  async function importDeck(): Promise<void> {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Presentations", extensions: DECK_EXTENSIONS }],
    });
    if (!picked || Array.isArray(picked)) return;

    setBusy(true);
    const deck = titleFromPath(picked);
    try {
      // PowerPoint becomes a PDF first; a PDF is handed straight back, so this
      // is one action either way and the operator never has to know which.
      setStatus("Preparing…");
      const pdfPath = await deckAsPdf(picked);

      // Read it back through the asset protocol, which the backend has just
      // allowed for this file. No second file-reading permission is needed.
      const bytes = await (await fetch(convertFileSrc(pdfPath))).arrayBuffer();
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
        <h2 className="panel-title">Slides / PowerPoint</h2>
        <button className="btn btn-sm" onClick={() => void importDeck()} disabled={busy}>
          {busy ? "Importing…" : "Import deck"}
        </button>
      </div>
      <p className="text-xs text-[var(--faint)]">
        PowerPoint (.pptx), OpenDocument (.odp) and PDF. Pages become images in Media, so
        they preview, project, step page by page, join a service order and run in the
        announcements loop, and they are still there next Sunday.
      </p>

      <div className="flex flex-wrap items-center gap-2 text-xs">
        {converters.length > 0 ? (
          <span className="text-[var(--muted)]">
            PowerPoint converted with <strong>{converters[0][0]}</strong>
            {converters.length > 1 && ` (+${converters.length - 1} other)`}
          </span>
        ) : (
          <span className="text-[var(--muted)]">
            No office suite found, so .pptx cannot be converted here. Install LibreOffice or
            ONLYOFFICE, or export the deck to PDF.
          </span>
        )}
        <button onClick={() => void chooseConverter()} className="btn btn-sm">
          {converters.length > 0 ? "Change" : "Find it myself"}
        </button>
      </div>

      {status && <p className="text-sm text-[var(--muted)]">{status}</p>}
    </section>
  );
}
