import { useEffect, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { libraryReady } from "../api";

const POLL_MS = 400;

interface ReadyGateProps {
  children: ReactNode;
}

/**
 * Holds the UI back until the backend has finished seeding the bundled library.
 * On a fresh install that takes a while (~270 MB of scripture), and every
 * command that touches the database blocks until it's done — so rendering the
 * console early would only produce a frozen window or failed calls.
 */
export function ReadyGate({ children }: ReadyGateProps) {
  const [ready, setReady] = useState(false);
  const [status, setStatus] = useState("Preparing your library…");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;

    async function poll(): Promise<void> {
      try {
        if (await libraryReady()) {
          if (!cancelled) setReady(true);
          return;
        }
      } catch {
        // The very first ticks can land before the backend registers its state.
        // Nothing to report — just keep asking.
      }
      if (!cancelled) timer = window.setTimeout(() => void poll(), POLL_MS);
    }
    void poll();

    const progress = listen<string>("library-progress", (e) => {
      if (!cancelled) setStatus(e.payload);
    });
    const done = listen("library-ready", () => {
      if (!cancelled) setReady(true);
    });
    const failed = listen<string>("library-error", (e) => {
      if (!cancelled) setError(e.payload);
    });

    return () => {
      cancelled = true;
      if (timer) window.clearTimeout(timer);
      void progress.then((un) => un());
      void done.then((un) => un());
      void failed.then((un) => un());
    };
  }, []);

  if (error) {
    return (
      <div className="flex min-h-screen items-center justify-center p-8">
        <div className="card max-w-md space-y-3 text-center">
          <h1 className="panel-title">Setup problem</h1>
          <p className="text-sm text-red-500">{error}</p>
          <p className="text-sm text-[var(--muted)]">
            Some of the library may be missing. You can continue — songs and any
            translations that did install will still work.
          </p>
          <button className="btn" onClick={() => setError(null)}>
            Continue anyway
          </button>
        </div>
      </div>
    );
  }

  if (!ready) {
    return (
      <div className="flex min-h-screen items-center justify-center p-8">
        <div className="space-y-2 text-center">
          <h1 className="text-lg font-bold tracking-tight">Bible · Operator Console</h1>
          <p className="text-sm text-[var(--muted)]">{status}</p>
          <p className="text-xs text-[var(--muted)]">First run only — this takes a minute.</p>
        </div>
      </div>
    );
  }

  return <>{children}</>;
}
