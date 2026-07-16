import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  acceptProposal,
  learnNow,
  learningStatus,
  rejectProposal,
  resetProfileToBaked,
  rollbackProfile,
  stopLearning,
  type LearnProgress,
  type LearningStatus,
  type Proposal,
} from "../api";

/** How much better (or worse) the proposal is, in the operator's terms. */
function verdict(p: Proposal): string {
  const gain = p.newScore - p.incumbentScore;
  if (gain > 0) {
    return `finds ${p.newScore} of ${p.references} references where the current settings find ${p.incumbentScore}`;
  }
  if (p.newAliases.length > 0) {
    return `no change to the settings — but it learnt ${p.newAliases.length} name${
      p.newAliases.length === 1 ? "" : "s"
    } the app was mishearing`;
  }
  return "nothing to change";
}

/**
 * Learning from the church's own services. Everything here is on this machine, needs
 * the operator's word before it changes anything, and can be put back afterwards —
 * either to the version before the last change, or to exactly what the app shipped.
 */
export function LearningPanel() {
  const [status, setStatus] = useState<LearningStatus | null>(null);
  const [progress, setProgress] = useState<LearnProgress | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // The operator said "not now": don't keep offering it this sitting.
  const dismissed = useRef(false);

  function load(): void {
    void learningStatus()
      .then(setStatus)
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)));
  }

  useEffect(() => {
    load();
    const unlisteners = [
      listen<LearnProgress>("learn-idle-progress", (e) => setProgress(e.payload)),
      listen<Proposal>("learn-proposal", () => {
        setProgress(null);
        load();
      }),
      listen<string>("learn-idle-done", (e) => {
        setProgress(null);
        setNote(e.payload);
        load();
      }),
      listen<string>("learn-error", (e) => {
        setProgress(null);
        setError(e.payload);
        load();
      }),
      // Whatever the operator does with the app changes what learning is allowed to do.
      listen("listen-started", load),
      listen("listen-stopped", load),
      listen("profile-changed", load),
    ];
    return () => {
      unlisteners.forEach((u) => void u.then((f) => f()));
    };
  }, []);

  async function act(what: () => Promise<unknown>): Promise<void> {
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      await what();
      load();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  if (!status) return null;
  const { proposal } = status;

  // Nothing recorded and approved, nothing proposed, nothing to undo: the church has
  // not opted into any of this, so the console says nothing about it.
  if (status.approved === 0 && !proposal && !status.canRollback && !status.running) return null;

  return (
    <section className="space-y-2 rounded-lg border p-3" style={{ background: "var(--surface-2)" }}>
      <div className="flex flex-wrap items-center gap-x-2">
        <h3 className="panel-title">Improving {status.profile}</h3>
        <span className="ml-auto text-xs text-[var(--muted)]">
          {status.approved} approved service{status.approved === 1 ? "" : "s"} · nothing leaves this
          machine
        </span>
      </div>

      {error && <p className="rounded bg-red-50 p-2 text-sm text-red-700">{error}</p>}
      {note && <p className="text-xs text-[var(--muted)]">{note}</p>}

      {status.running ? (
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <span className="text-sm">{progress?.stage ?? "Learning…"}</span>
            <span className="text-xs text-[var(--muted)]">{progress?.percent ?? 0}%</span>
            <button onClick={() => void act(stopLearning)} className="btn btn-sm ml-auto">
              Stop
            </button>
          </div>
          <div className="h-1.5 w-full rounded" style={{ background: "var(--surface)" }}>
            <div
              className="h-1.5 rounded"
              style={{ width: `${progress?.percent ?? 0}%`, background: "var(--live)" }}
            />
          </div>
          <p className="text-xs text-[var(--muted)]">
            It stops on its own the moment you start a service or put anything on the screen.
          </p>
        </div>
      ) : proposal ? (
        <div className="space-y-1.5 rounded border p-2" style={{ background: "var(--surface)" }}>
          <p className="text-sm">
            From {Math.round(proposal.minutes)} minutes across {proposal.sessions.length} service
            {proposal.sessions.length === 1 ? "" : "s"}, it {verdict(proposal)}.
          </p>
          {proposal.settings !== "unchanged" && (
            <p className="text-xs text-[var(--muted)]">Proposed settings: {proposal.settings}</p>
          )}
          {proposal.newAliases.length > 0 && (
            <p className="text-xs text-[var(--muted)]">
              New misheard names: {proposal.newAliases.map((a) => `“${a}”`).join(", ")}
            </p>
          )}
          <div className="flex flex-wrap gap-1.5">
            <button
              onClick={() => void act(acceptProposal)}
              disabled={busy}
              className="btn btn-sm btn-primary"
              title="Use these — you can put it back afterwards"
            >
              Use this
            </button>
            <button onClick={() => void act(rejectProposal)} disabled={busy} className="btn btn-sm">
              No thanks
            </button>
          </div>
        </div>
      ) : status.blocked ? (
        <p className="text-xs text-[var(--muted)]">{status.blocked}</p>
      ) : (
        !dismissed.current && (
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="text-sm">
              There {status.approved === 1 ? "is" : "are"} {status.approved} approved service
              {status.approved === 1 ? "" : "s"} to learn from. It takes a while, and stops the
              moment you need the machine.
            </span>
            <button onClick={() => void act(learnNow)} disabled={busy} className="btn btn-sm btn-primary">
              Learn now
            </button>
            <button
              onClick={() => {
                dismissed.current = true;
                setNote("Not now — it will offer again later.");
              }}
              className="btn btn-sm"
            >
              Not now
            </button>
          </div>
        )
      )}

      {(status.canRollback || status.canReset) && (
        <div className="flex flex-wrap items-center gap-1.5 border-t pt-1.5">
          <span className="text-xs text-[var(--muted)]">If it got worse:</span>
          {status.canRollback && (
            <button
              onClick={() => void act(rollbackProfile)}
              disabled={busy}
              className="btn btn-sm"
              title="Put back the settings in force before the last change"
            >
              Undo last change
            </button>
          )}
          {status.canReset && (
            <button
              onClick={() => {
                if (
                  window.confirm(
                    `Put ${status.profile} back exactly as the app shipped, discarding everything this machine has learnt about them?`,
                  )
                ) {
                  void act(resetProfileToBaked);
                }
              }}
              disabled={busy}
              className="btn btn-sm"
              title="Discard all local learning for this speaker"
            >
              Reset to as shipped
            </button>
          )}
        </div>
      )}
    </section>
  );
}
