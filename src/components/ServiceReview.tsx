import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  approveSession,
  discardSession,
  reviewSessions,
  type Moment,
  type SessionSummary,
} from "../api";

// What happened at each moment, in the operator's words. These describe the service —
// they are how you judge whether it went well enough to be worth learning from. They
// are not a lever on the learning itself: it listens to the whole recording and works
// out what was read aloud for itself.
const KINDS: Record<Moment["kind"], { label: string; hint: string; tone: string }> = {
  confirmed: {
    label: "confirmed",
    hint: "The app found it and the preacher read it aloud.",
    tone: "tint-good",
  },
  corrected: {
    label: "corrected",
    hint: "The app put up the wrong verse and you replaced it.",
    tone: "tint-warn",
  },
  operator: {
    label: "you chose",
    hint: "You projected this yourself.",
    tone: "tint-info",
  },
  auto: {
    label: "auto",
    hint: "The app projected this on its own and nothing confirmed it.",
    tone: "tint-neutral",
  },
};

/** "3:05 pm, 16 Jul" from the service's timestamp name. */
function when(name: string): string {
  const ms = Number(name);
  if (!Number.isFinite(ms) || ms <= 0) return "recorded service";
  return new Date(ms).toLocaleString(undefined, {
    weekday: "short",
    day: "numeric",
    month: "short",
    hour: "numeric",
    minute: "2-digit",
  });
}

/** A stable identity for one moment within its service, so two moments naming the same
 *  verse stay distinct rows. */
const momentKey = (m: Moment, i: number): string => `${i}-${m.kind}-${m.reference}`;

/**
 * The end-of-service review. A recorded service teaches the app nothing until the
 * operator has looked at what it captured and approved it — a service where the app
 * did badly, or where the wrong person was on the microphone, is one to throw away.
 *
 * The choice is over the whole service, not moment by moment: learning listens to the
 * recording and works out what was read aloud for itself, so there is no such thing as
 * approving half of one. The moments are shown as what happened, to decide on.
 * Everything here is on this machine.
 */
export function ServiceReview() {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [open, setOpen] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  function load(): void {
    void reviewSessions()
      .then(setSessions)
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)));
  }

  useEffect(() => {
    load();
    // A service that just ended is the one most worth reviewing.
    const stopped = listen("listen-stopped", load);
    return () => {
      void stopped.then((f) => f());
    };
  }, []);

  const pending = sessions.filter((s) => !s.approved);
  if (sessions.length === 0) return null;

  async function approve(s: SessionSummary): Promise<void> {
    setBusy(s.name);
    setError(null);
    try {
      await approveSession(s.name);
      load();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  }

  async function discard(s: SessionSummary): Promise<void> {
    if (!window.confirm(`Delete the recording from ${when(s.name)}? This cannot be undone.`)) return;
    setBusy(s.name);
    setError(null);
    try {
      await discardSession(s.name);
      load();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="space-y-2 rounded-lg border p-3" style={{ background: "var(--surface-2)" }}>
      <div className="flex flex-wrap items-center gap-x-2">
        <h3 className="panel-title">Recorded services</h3>
        {pending.length > 0 && (
          <span className="tint-strong tint-warn rounded px-1.5 py-0.5 text-[10px]">
            {pending.length} to review
          </span>
        )}
        <span className="ml-auto text-xs text-[var(--muted)]">
          Stays on this machine. Nothing is learned until you approve it.
        </span>
      </div>

      {error && <p className="tint tint-bad rounded p-2 text-sm">{error}</p>}

      {sessions.map((s) => {
        const wrong = s.moments.filter((m) => m.kind === "corrected").length;
        return (
          <div key={s.name} className="rounded border" style={{ background: "var(--surface)" }}>
            <div className="flex flex-wrap items-center gap-2 p-2">
              <button
                onClick={() => setOpen(open === s.name ? null : s.name)}
                className="text-sm font-semibold"
                title="Show what happened during this service"
              >
                {open === s.name ? "▾" : "▸"} {when(s.name)}
              </button>
              <span className="text-xs text-[var(--muted)]">
                {s.minutes < 1 ? "under a minute" : `${Math.round(s.minutes)} min`} ·{" "}
                {s.moments.length} verse{s.moments.length === 1 ? "" : "s"}
                {wrong > 0 ? ` · you corrected ${wrong}` : ""}
              </span>
              {s.approved ? (
                <span className="tint-strong tint-good ml-auto rounded px-1.5 py-0.5 text-[10px]">
                  ✓ approved
                </span>
              ) : (
                <span className="ml-auto flex gap-1.5">
                  <button
                    onClick={() => void approve(s)}
                    disabled={busy === s.name}
                    className="btn btn-sm btn-primary"
                    title="Keep this service and let the app learn from it"
                  >
                    Approve
                  </button>
                  <button
                    onClick={() => void discard(s)}
                    disabled={busy === s.name}
                    className="btn btn-sm"
                    title="Delete this recording"
                  >
                    Discard
                  </button>
                </span>
              )}
            </div>

            {open === s.name && (
              <div className="space-y-1 border-t p-2">
                {s.moments.length === 0 ? (
                  <p className="text-xs text-[var(--muted)]">
                    No scripture went on the screen during this service. If nothing was read
                    aloud either, there is nothing here to learn from.
                  </p>
                ) : (
                  s.moments.map((m, i) => {
                    const kind = KINDS[m.kind] ?? KINDS.auto;
                    return (
                      <div key={momentKey(m, i)} className="flex items-start gap-2 rounded p-1.5 text-xs">
                        <span
                          className={`tint-strong rounded px-1.5 py-0.5 text-[10px] ${kind.tone}`}
                          title={kind.hint}
                        >
                          {kind.label}
                        </span>
                        <span className="min-w-0 flex-1">
                          <span className="font-semibold">{m.reference}</span>
                          {m.replaced && (
                            <span className="text-[var(--muted)]"> · replaced {m.replaced}</span>
                          )}
                          {m.spoken && (
                            <span className="block truncate text-[var(--muted)]" title={m.spoken}>
                              “{m.spoken}”
                            </span>
                          )}
                        </span>
                      </div>
                    );
                  })
                )}
              </div>
            )}
          </div>
        );
      })}
    </section>
  );
}
