/**
 * Sending sound to a chosen output device.
 *
 * Windows routes an application's audio to the system default playback device and
 * takes no notice of which monitor the window is on. So projecting onto a TV over
 * HDMI puts the picture on the TV and leaves the sound in the laptop's speakers.
 * `setSinkId` is the only way to move it, and it has to be applied to each media
 * element, every time one is mounted.
 */

/** A playback device the operator can send sound to. */
export interface SoundOutput {
  id: string;
  label: string;
}

/** An element whose output can be redirected. Typed here because `setSinkId` is
 *  still absent from the DOM lib in some TypeScript versions. */
type Redirectable = HTMLMediaElement & {
  setSinkId?: (id: string) => Promise<void>;
  sinkId?: string;
};

export function canChooseOutput(): boolean {
  return (
    typeof HTMLMediaElement !== "undefined" &&
    "setSinkId" in HTMLMediaElement.prototype &&
    typeof navigator !== "undefined" &&
    !!navigator.mediaDevices?.enumerateDevices
  );
}

/**
 * Point one element at a device. An empty id restores the system default.
 *
 * Failure is deliberately quiet at the call site's option: a device can be
 * unplugged between being chosen and being used, and when that happens the right
 * outcome is sound from the default device rather than a service with no sound and
 * an error nobody is looking at. Returns whether it took, so a settings screen can
 * still say so.
 */
export async function applyOutput(el: HTMLMediaElement | null, id: string): Promise<boolean> {
  const target = el as Redirectable | null;
  if (!target?.setSinkId) return false;
  // Re-applying the same sink is not free, and this runs on every state change.
  if ((target.sinkId ?? "") === id) return true;
  try {
    await target.setSinkId(id);
    return true;
  } catch {
    return false;
  }
}

/**
 * The playback devices this machine offers.
 *
 * Device *names* are withheld by the browser until the page has been granted
 * microphone permission — without it the ids are present but the labels come back
 * empty, which is useless to an operator choosing between them. That permission is
 * not requested here: see `revealOutputNames`, which the operator triggers on
 * purpose. Devices whose name is still hidden are returned with a placeholder rather
 * than dropped, so a machine that never grants it can still be configured by trial.
 */
export async function listOutputs(): Promise<SoundOutput[]> {
  if (!navigator.mediaDevices?.enumerateDevices) return [];
  const devices = await navigator.mediaDevices.enumerateDevices();
  return devices
    .filter((d) => d.kind === "audiooutput")
    .map((d, i) => ({
      id: d.deviceId,
      label: d.label || (d.deviceId === "default" ? "System default" : `Sound output ${i + 1}`),
    }));
}

/** Do the devices still have no names? Then `revealOutputNames` is worth offering. */
export function namesHidden(outputs: SoundOutput[]): boolean {
  return outputs.some((o) => o.label.startsWith("Sound output "));
}

/**
 * Unlock the device names.
 *
 * The browser ties the right to read playback device names to microphone
 * permission, so there is no way to show "Samsung TV (NVIDIA High Definition
 * Audio)" instead of "Sound output 3" without asking for the microphone once. The
 * stream is stopped on the very next line and nothing is recorded from it.
 *
 * This is deliberately not done on load. The app is careful about the room
 * microphone elsewhere for good reasons, and opening it — even for an instant, even
 * only to read a list of names — should be something the operator asked for.
 */
export async function revealOutputNames(): Promise<SoundOutput[]> {
  if (!navigator.mediaDevices?.getUserMedia) return listOutputs();
  const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  stream.getTracks().forEach((t) => t.stop());
  return listOutputs();
}

/**
 * The remembered device, matched up against what is actually plugged in.
 *
 * Ids do not always survive a reinstall, so the stored name is the fallback: a TV
 * that comes back with a new id is still recognisable by what it is called. Returns
 * an empty id when neither matches, which means the system default — the correct
 * answer for a device that is genuinely no longer there.
 */
export function resolveRemembered(
  outputs: SoundOutput[],
  remembered: SoundOutput,
): string {
  if (!remembered.id) return "";
  if (outputs.some((o) => o.id === remembered.id)) return remembered.id;
  if (remembered.label) {
    const byName = outputs.find((o) => o.label === remembered.label);
    if (byName) return byName.id;
  }
  return "";
}
