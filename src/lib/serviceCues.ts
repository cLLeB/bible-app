import type { Cue } from "../services";

/**
 * Keep only run-order cues that can actually be projected.
 *
 * Stored run orders outlive the builds that wrote them. An older build saved
 * verse cues without `bookOsis`, which cannot be projected, and a cue that
 * fails at the moment the operator clicks it is worse than one that was never
 * offered. Anything unrecognised is dropped for the same reason.
 *
 * The cost of getting this wrong in the other direction is just as real: drop a
 * shape too eagerly and a service order silently loses items between releases.
 */
export function usableCues(raw: readonly unknown[]): Cue[] {
  return raw.filter((value): value is Cue => {
    if (!value || typeof value !== "object") return false;
    const cue = value as Partial<Cue> & { type?: string };
    switch (cue.type) {
      case "verse":
        return Boolean((cue as Extract<Cue, { type: "verse" }>).verse?.bookOsis);
      case "song":
        return (cue as Extract<Cue, { type: "song" }>).songId != null;
      case "media":
        return (cue as Extract<Cue, { type: "media" }>).mediaId != null;
      default:
        return false;
    }
  });
}
