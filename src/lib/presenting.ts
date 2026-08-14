/** Who drove a presentation, as reported by the backend's `presenting-changed`. */
export type PresentingSource = "console" | "remote" | "voice";

/**
 * Should the console apply a backend presentation to its own state?
 *
 * Console-driven changes are already applied by whatever made them, and
 * re-applying one would hand scripture the keyboard in the middle of a service
 * order that already owns it. Everything else, the phone remote and the
 * listening loop, has no other way into the console's state.
 */
export function mirrorsToConsole(source: string): boolean {
  return source !== "console";
}
