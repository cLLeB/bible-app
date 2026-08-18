import { useCallback, useEffect, useState } from "react";
import { AudioInputPicker } from "./AudioInputPicker";
import { CalibrationPanel } from "./CalibrationPanel";
import { LearningPanel } from "./LearningPanel";
import { ServiceReview } from "./ServiceReview";
import { appFlavor, audioInputs, listeningEnabled, type SttModel } from "../api";

/**
 * Everything about hearing the preacher that is set once rather than touched during
 * a service: which sound input to listen on, whose voice the app is tuned for, what
 * it has learned, and the services it has recorded.
 *
 * All of this used to sit inside the Live listening card, folded away but still
 * there - about 1,300 lines of setup on the surface an operator works from mid
 * service. Live now carries only Start/Stop, the transcript and the detected
 * verses; the rest is here.
 *
 * The auto-project rules deliberately stayed on Live. They are the one thing in the
 * old fold that an operator does change during a service, when the app is being too
 * eager or too shy.
 */
export function VoiceSetupPanel() {
  const [model, setModel] = useState<SttModel>("small");
  const [listening, setListening] = useState(false);
  const [needsInput, setNeedsInput] = useState(false);
  const [reviewKey, setReviewKey] = useState(0);

  const refreshInput = useCallback((): void => {
    void audioInputs()
      .then((i) => setNeedsInput(i.chosen === null))
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    void appFlavor().then((f) => setModel(f.defaultModel)).catch(() => undefined);
    // Most of this must not be changed while the microphone is open, so the panel
    // needs to know. Asked rather than assumed, because this panel can be opened
    // long after listening started.
    void listeningEnabled().then(setListening).catch(() => undefined);
    refreshInput();
  }, [refreshInput]);

  function profileChanged(): void {
    setReviewKey((k) => k + 1);
  }

  return (
    <section className="space-y-3">
      <h2 className="panel-title">Voice</h2>

      {listening && (
        <p className="tint tint-warn rounded px-2 py-1 text-sm">
          Listening now. Stop it before changing these.
        </p>
      )}

      <AudioInputPicker disabled={listening} onChanged={refreshInput} />
      {needsInput && (
        <p className="text-sm text-[var(--muted)]">
          Nothing is chosen yet, so the app will not listen.
        </p>
      )}

      <CalibrationPanel model={model} disabled={listening} onProfileChange={profileChanged} />
      <ServiceReview key={`review-${reviewKey}`} />
      <LearningPanel key={`learning-${reviewKey}`} />
    </section>
  );
}
