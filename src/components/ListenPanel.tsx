import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AudioInputPicker } from "./AudioInputPicker";
import { CalibrationPanel } from "./CalibrationPanel";
import { LearningPanel } from "./LearningPanel";
import { ServiceReview } from "./ServiceReview";
import {
  appFlavor,
  audioInputs,
  blankProjection,
  recordChoice,
  recordMoment,
  recordingConsent,
  recordingEnabled,
  setRecording,
  startListening,
  stopListening,
  type Candidate,
  type Moment,
  type SttModel,
  type VersePayload,
} from "../api";
import { present } from "../present";
import { useServiceStore, type Cue } from "../services";

// A medium-confidence hearing that lands on a chapter already on the run sheet is
// certain enough to project on its own: the expectation resolves the doubt. Below this
// floor the hearing itself was too weak to trust, even with that corroboration — so we
// still only suggest it and let the operator decide.
const RUN_SHEET_ASSIST_FLOOR = 0.72;

// Is this verse's chapter one the operator has queued on the run sheet? Book + chapter,
// not the exact verse: the preacher going to that chapter is the corroboration that
// matters, and she may read a different verse within it than the one queued.
function onRunSheet(cues: Cue[], v: { bookOsis: string; chapter: number }): boolean {
  return cues.some(
    (c) => c.type === "verse" && c.verse.bookOsis === v.bookOsis && c.verse.chapter === v.chapter,
  );
}

export function ListenPanel() {
  const [listening, setListening] = useState(false);
  // Fixed by this build's flavor (base-personal / small-personal), not operator-switchable.
  const [model, setModel] = useState<SttModel>("base");
  const [lines, setLines] = useState<string[]>([]);
  const [candidates, setCandidates] = useState<Candidate[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [confirmed, setConfirmed] = useState<string | null>(null);
  const [alternatives, setAlternatives] = useState<VersePayload[]>([]);
  const [autoProject, setAutoProject] = useState(() => localStorage.getItem("auto-project") === "1");
  const [threshold, setThreshold] = useState(() => {
    const s = Number(localStorage.getItem("auto-threshold"));
    return s >= 50 && s <= 100 ? s / 100 : 0.82;
  });
  const [useRunSheet, setUseRunSheet] = useState(() => localStorage.getItem("use-run-sheet") === "1");
  const cues = useServiceStore((s) => s.cues);
  const addVerse = useServiceStore((s) => s.addVerse);
  const runSheetVerses = cues.filter((c) => c.type === "verse").length;

  function changeAuto(v: boolean): void {
    setAutoProject(v);
    localStorage.setItem("auto-project", v ? "1" : "0");
  }
  function changeRunSheet(v: boolean): void {
    setUseRunSheet(v);
    localStorage.setItem("use-run-sheet", v ? "1" : "0");
  }
  function changeThreshold(pct: number): void {
    const v = Math.min(100, Math.max(50, pct || 82));
    setThreshold(v / 100);
    localStorage.setItem("auto-threshold", String(v));
  }

  // This build's flavor decides the model; the operator doesn't choose it.
  useEffect(() => {
    void appFlavor().then((f) => setModel(f.defaultModel));
  }, []);

  // Until a sound input is chosen the app will not listen at all, so that setting is
  // not something to hide behind a fold — it is the whole job.
  const [needsInput, setNeedsInput] = useState(false);
  // Listening to this laptop's own microphone is a legitimate thing to do — it is how
  // you show someone what the app does — but it must never be a quiet state. The
  // picker itself lives behind the fold below, so the fact is repeated out here where
  // the operator is already looking.
  const [onRoomMic, setOnRoomMic] = useState(false);
  const refreshInput = useCallback((): void => {
    void audioInputs()
      .then((i) => {
        setNeedsInput(i.chosen === null);
        setOnRoomMic(i.onRoomMic);
      })
      .catch(() => undefined);
  }, []);
  useEffect(refreshInput, [listening, refreshInput]);

  // The folded-away auto-project rules still decide whether verses appear by
  // themselves, so the header says where they stand without being asked.
  const autoSummary = !autoProject
    ? "You project everything"
    : `Projects by itself at ${Math.round(threshold * 100)}%${useRunSheet ? " · run sheet trusted" : ""}`;

  const [recording, setRecordingState] = useState(false);
  // Whether today's speaker has agreed to be recorded at all. Without that there is
  // no switch to offer: the app does not ask the operator to decide for the preacher.
  const [consent, setConsent] = useState(false);
  const [reviewKey, setReviewKey] = useState(0);

  function loadRecording(): void {
    void recordingEnabled().then(setRecordingState).catch(() => undefined);
    void recordingConsent().then(setConsent).catch(() => undefined);
  }
  useEffect(loadRecording, []);

  // The speaker (or their answer) changed: re-read both, and rebuild the panels that
  // are about that speaker's recordings.
  function profileChanged(): void {
    loadRecording();
    setReviewKey((k) => k + 1);
  }

  function changeRecording(v: boolean): void {
    setRecordingState(v);
    setRecording(v).catch((err: unknown) => {
      // Refused — almost certainly consent was withdrawn elsewhere. Show the truth.
      setRecordingState(false);
      setError(err instanceof Error ? err.message : String(err));
    });
  }

  // Refs so the once-registered event listener sees current values.
  const autoRef = useRef(false);
  autoRef.current = autoProject;
  const thresholdRef = useRef(0.82);
  thresholdRef.current = threshold;
  const runSheetRef = useRef(false);
  runSheetRef.current = useRunSheet;
  const recordingRef = useRef(false);
  recordingRef.current = recording;
  // Latest hearing, so a moment can say what the speaker was saying at the time.
  const spokenRef = useRef("");

  // The last verse the app projected on its own. If the operator then picks a
  // different one, that pick is a correction of this — the most valuable label we
  // can collect, because it names both the mistake and the right answer.
  const lastAutoRef = useRef<{ reference: string; spoken: string } | null>(null);

  // Moments are only meaningful for a service that is being recorded; with the
  // toggle off nothing about the service is kept, and this must keep that promise.
  function logMoment(kind: Moment["kind"], v: VersePayload, extra: Partial<Moment> = {}): void {
    if (!recordingRef.current) return;
    void recordMoment({
      kind,
      reference: v.reference,
      bookOsis: v.bookOsis,
      chapter: v.chapter,
      verse: v.verse,
      confidence: 0,
      source: "",
      spoken: spokenRef.current,
      replaced: "",
      ...extra,
    }).catch(() => undefined); // a lost label must never disturb the service
  }

  useEffect(() => {
    const unlisteners = [
      listen<string>("transcript", (e) => {
        spokenRef.current = e.payload;
        setLines((prev) => [e.payload, ...prev].slice(0, 6));
      }),
      listen<Candidate>("verse-candidate", (e) => {
        const c = e.payload;
        setCandidates((prev) => [c, ...prev].slice(0, 12));
        if (!autoRef.current || c.source === "voice-nav") return;
        // Confident enough on its own → project.
        if (c.confidence >= thresholdRef.current) {
          autoProjected(c);
          return;
        }
        // Not sure enough on the hearing alone — but if it lands on a chapter the
        // operator queued on the run sheet, that expectation makes it certain. Only a
        // decent hearing qualifies (>= floor), and only a match to the run sheet: a
        // detection off the run sheet is never forced, and a poor hearing stays a mere
        // suggestion. Better to leave it to the operator than to project a wrong verse.
        if (
          runSheetRef.current &&
          c.confidence >= RUN_SHEET_ASSIST_FLOOR &&
          onRunSheet(useServiceStore.getState().cues, c.verse)
        ) {
          autoProjected(c);
        }
      }),
      // The speaker confirmed a suggested verse (said "yes"/"amen" or read it
      // aloud) — project it and mark it confirmed.
      listen<VersePayload>("verse-confirmed", (e) => {
        setConfirmed(e.payload.reference);
        void present(e.payload);
        // The speaker himself vouched for this one, so it is no longer an
        // unconfirmed guess the operator might be about to correct.
        logMoment("confirmed", e.payload, { spoken: lastAutoRef.current?.spoken ?? spokenRef.current });
        lastAutoRef.current = null;
      }),
      // Standby alternatives for the current best guess — operator can pick one.
      listen<VersePayload[]>("verse-alternatives", (e) => {
        setAlternatives(e.payload);
      }),
      listen("listen-started", () => {
        setListening(true);
        setError(null);
      }),
      listen("listen-stopped", () => setListening(false)),
      listen("listen-idle-stop", () => {
        setListening(false);
        setError("Stopped listening after 20 minutes of silence. Click to start again.");
      }),
      listen<string>("listen-error", (e) => setError(e.payload)),
    ];
    return () => {
      unlisteners.forEach((u) => u.then((f) => f()));
    };
  }, []);

  // The app decided this one by itself. Remember it: if the operator overrides it in
  // a moment, that override is a labelled mistake worth more than the guess.
  function autoProjected(c: Candidate): void {
    void present(c.verse);
    logMoment("auto", c.verse, { confidence: c.confidence, source: c.source });
    lastAutoRef.current = { reference: c.verse.reference, spoken: spokenRef.current };
  }

  // Present a verse and teach the ranker which one the operator chose for the
  // current spoken description.
  function pick(v: VersePayload): void {
    void present(v);
    if (lines[0]) {
      void recordChoice(lines[0], v.bookOsis, v.chapter, v.verse);
    }
    // Replacing what the app had just put on screen is a correction; anything else
    // is the operator simply driving.
    const auto = lastAutoRef.current;
    if (auto && auto.reference !== v.reference) {
      logMoment("corrected", v, { replaced: auto.reference, spoken: auto.spoken });
    } else {
      logMoment("operator", v);
    }
    lastAutoRef.current = null;
  }

  async function toggle(): Promise<void> {
    try {
      if (listening) {
        await stopListening();
        setListening(false);
      } else {
        setError(null);
        await startListening(model);
        setListening(true);
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <section className="space-y-3">
      {/* What is happening right now, and the one button that changes it. Everything
          the operator only touches before or after a service lives further down. */}
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <h2 className="panel-title">Live listening</h2>
        {listening && (
          <span className="chip" style={{ color: "var(--live)" }}>
            ● listening…
          </span>
        )}
        <span className="ml-auto text-xs text-[var(--muted)]">{autoSummary}</span>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <button
          onClick={toggle}
          className={`btn btn-lg ${listening ? "" : "btn-primary"}`}
          style={listening ? { background: "var(--danger)", borderColor: "transparent", color: "#fff" } : undefined}
        >
          {listening ? "■ Stop listening" : "● Start listening"}
        </button>
        {consent && (
          <label
            className="flex items-center gap-1.5 text-sm text-[var(--muted)]"
          >
            <input
              type="checkbox"
              checked={recording}
              disabled={listening}
              onChange={(e) => changeRecording(e.target.checked)}
            />
            Record service
          </label>
        )}
      </div>

      {error && <p className="tint tint-bad rounded-lg p-2 text-sm">{error}</p>}

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div>
          <div className="panel-title mb-1.5">Transcript</div>
          <div className="min-h-24 space-y-1 rounded-lg border p-2.5 text-sm" style={{ background: "var(--surface-2)" }}>
            {lines.length === 0 ? (
              <span className="text-[var(--faint)]">Speak a reference, e.g. “John chapter 3 verse 16”.</span>
            ) : (
              // The newest line is the one being acted on; the rest have had their turn.
              lines.map((l, i) => (
                <p key={i} className={i === 0 ? "text-[var(--text)]" : "text-[var(--faint)]"}>
                  {l}
                </p>
              ))
            )}
          </div>
        </div>

        <div>
          <div className="mb-1.5 flex items-center justify-between">
            <span className="panel-title">Detected verses</span>
            <button onClick={() => blankProjection()} className="btn btn-sm">
              Blank
            </button>
          </div>
          {confirmed && (
            <p className="tint tint-good mb-1 rounded px-2 py-1 text-xs">
              ✓ Confirmed by speaker: {confirmed}
            </p>
          )}
          {alternatives.length > 0 && (
            <div className="mb-1 rounded border border-dashed p-1.5" style={{ borderColor: "var(--border)" }}>
              <div className="mb-1 text-[10px] uppercase text-[var(--faint)]">Or did you mean…</div>
              <div className="flex flex-wrap gap-1">
                {alternatives.map((alt, i) => (
                  <button
                    key={`${alt.reference}-${i}`}
                    onClick={() => pick(alt)}
                    className="tint tint-neutral pick-hover rounded px-2 py-0.5 text-xs"
                    title={alt.text}
                  >
                    {alt.reference}
                  </button>
                ))}
              </div>
            </div>
          )}
          <div className="min-h-24 space-y-1">
            {candidates.length === 0 ? (
              <span className="text-sm text-[var(--faint)]">Detected references appear here.</span>
            ) : (
              candidates.map((c, i) => (
                // Two controls, not one: tapping the body projects the verse,
                // while ＋ Service only queues it. A button cannot be nested
                // inside a button, so the row is a container rather than the
                // single big button it used to be.
                <div
                  key={`${c.verse.reference}-${i}`}
                  className="flex items-stretch gap-1 rounded border"
                  style={{ borderColor: "var(--border)" }}
                >
                  <button
                    onClick={() => pick(c.verse)}
                    className="pick-hover min-w-0 flex-1 rounded-l p-2 text-left"
                    title="Project this verse now"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-sm font-semibold">{c.verse.reference}</span>
                      <span className="flex flex-none items-center gap-1">
                        {onRunSheet(cues, c.verse) && (
                          <span
                            className="tint-strong tint-info rounded px-1.5 py-0.5 text-[10px]"
                          >
                            on run sheet
                          </span>
                        )}
                        <span
                          className={`tint-strong rounded px-1.5 py-0.5 text-[10px] ${
                            c.confidence >= 0.9
                              ? "tint-good"
                              : c.confidence >= 0.8
                                ? "tint-warn"
                                : "tint-neutral"
                          }`}
                          title={`${c.source} match`}
                        >
                          {Math.round(c.confidence * 100)}% · {c.source}
                        </span>
                      </span>
                    </div>
                    <div className="line-clamp-2 text-xs text-[var(--muted)]">{c.verse.text}</div>
                  </button>
                  <button
                    onClick={() => addVerse(c.verse)}
                    className="pick-hover flex-none rounded-r border-l px-2 text-xs text-[var(--muted)]"
                    style={{ borderColor: "var(--border)" }}
                    title={`Add ${c.verse.reference} to the service order`}
                    aria-label={`Add ${c.verse.reference} to the service order`}
                  >
                    ＋
                  </button>
                </div>
              ))
            )}
          </div>
        </div>
      </div>

      {/* Everything below is set once and left alone. It opens itself when it is the
          thing standing between the operator and a working service — an unchosen
          sound input — and stays out of the way the rest of the time. */}
      <details className="rounded-lg border" style={{ borderColor: "var(--border)" }} open={needsInput}>
        <summary className="cursor-pointer px-3 py-2 text-sm font-medium">
          Before the service
          <span className="text-[var(--muted)]">
            {needsInput ? " · choose the sound input" : " · sound input, auto-project"}
          </span>
          {onRoomMic && (
            <span
              className="tint tint-warn ml-2 rounded border px-1.5 py-0.5 text-xs font-normal"
            >
              room mic
            </span>
          )}
        </summary>
        <div className="space-y-3 px-3 pb-3">
          <AudioInputPicker disabled={listening} onChanged={refreshInput} />

          <div className="space-y-1.5">
            <div className="panel-title">When to project by itself</div>
            <label
              className="flex items-center gap-1.5 text-sm"
            >
              <input
                type="checkbox"
                checked={autoProject}
                onChange={(e) => changeAuto(e.target.checked)}
              />
              Project a verse on its own when the app is at least
              <input
                type="number"
                min={50}
                max={100}
                value={Math.round(threshold * 100)}
                onChange={(e) => changeThreshold(Number(e.target.value))}
                disabled={!autoProject}
                className="input w-16 px-2 text-center"
              />
              % sure
            </label>
            <label
              className="flex items-center gap-1.5 text-sm"
            >
              <input
                type="checkbox"
                checked={useRunSheet}
                disabled={!autoProject}
                onChange={(e) => changeRunSheet(e.target.checked)}
              />
              Trust a fair match when its chapter is on the run sheet
              {runSheetVerses > 0 ? ` (${runSheetVerses} queued)` : ""}
            </label>
          </div>
        </div>
      </details>

      <CalibrationPanel model={model} disabled={listening} onProfileChange={profileChanged} />

      {/* Only after the service, when there is something to look back on. Keyed on the
          speaker so switching preacher never shows the previous one's services. */}
      {!listening && (
        <>
          <ServiceReview key={`review-${reviewKey}`} />
          <LearningPanel key={`learning-${reviewKey}`} />
        </>
      )}
    </section>
  );
}
