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

/**
 * This machine's own speakers, out of everything the OS offers.
 *
 * Used to pin the preview, which must play where the operator is sitting whatever
 * the congregation screen has been pointed at. Falling back to "the system default"
 * was nearly right and not safe: the default is whatever Windows says, and a church
 * that once set the TV as their default would have the preview talking through the
 * hall.
 *
 * Names are all there is to go on, so this reads them the way the input side already
 * reads microphone names. A display device is ruled out first - HDMI and DisplayPort
 * audio is how a TV appears - and then the onboard chip is looked for. Returns "" when
 * nothing can be identified, which means the system default: worse than a named
 * device, better than silence.
 */
export function laptopOutput(outputs: readonly SoundOutput[]): string {
  const named = outputs.filter((o) => o.id !== "default" && o.label);

  const isDisplay = (label: string): boolean => {
    const n = label.toLowerCase();
    return [
      "hdmi",
      "displayport",
      "display audio",
      "high definition audio", // how NVIDIA/AMD present a TV's audio
      "nvidia",
      "tv",
      "samsung",
      "lg ",
      "sony",
      "philips",
      "hisense",
      "monitor",
      "projector",
    ].some((hint) => n.includes(hint));
  };

  const isOnboard = (label: string): boolean => {
    const n = label.toLowerCase();
    return ["realtek", "internal", "built-in", "builtin", "smart sound", "laptop", "speakers"].some(
      (hint) => n.includes(hint),
    );
  };

  const onboard = named.find((o) => isOnboard(o.label) && !isDisplay(o.label));
  if (onboard) return onboard.id;
  // No onboard speakers named as such: take anything that is at least not a screen.
  const notAScreen = named.find((o) => !isDisplay(o.label));
  return notAScreen?.id ?? "";
}
