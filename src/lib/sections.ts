/**
 * Jumping straight to a section of a song, and editing one without taking it off
 * the screen.
 *
 * A song on the wall is a list of slides, and the operator follows the worship
 * leader rather than the printed order: back to the chorus, skip the second verse,
 * round again to the bridge. Scanning a list of slides for the right one is slower
 * than the leader, and the platforms that do this well put the sections themselves
 * within one click.
 */

export interface SlideLike {
  orderIndex: number;
  text: string;
  label: string | null;
}

export interface Section {
  label: string;
  /** Where to jump: the first slide carrying this label. */
  orderIndex: number;
}

/**
 * The sections worth offering as buttons, in the order they first appear.
 *
 * Deduped by label, because a chorus sung four times is one destination, not four,
 * and jumping to "Chorus" sensibly means its first slide. Unlabelled slides are left
 * out entirely: a button called "Slide 7" tells the operator nothing they cannot
 * already see in the list.
 */
export function sections(slides: readonly SlideLike[]): Section[] {
  const out: Section[] = [];
  for (const s of slides) {
    const label = s.label?.trim();
    if (!label) continue;
    if (out.some((x) => x.label.toLowerCase() === label.toLowerCase())) continue;
    out.push({ label, orderIndex: s.orderIndex });
  }
  return out;
}

/**
 * A short form for a section button: "Verse 1" is "V1", "Chorus" is "C".
 *
 * The buttons sit in a row above a live song and there may be eight of them, so the
 * full names would wrap and push the slide list off the screen. Anything unrecognised
 * keeps its own first word, which is better than inventing an abbreviation nobody
 * will read the same way twice.
 */
export function shortLabel(label: string): string {
  const m = /^([a-z\- ]+?)\s*(\d+)?$/i.exec(label.trim());
  if (!m) return label.slice(0, 3);
  const [, word, num = ""] = m;
  const key = word.trim().toLowerCase();
  const initials: Record<string, string> = {
    verse: "V",
    chorus: "C",
    "pre-chorus": "PC",
    prechorus: "PC",
    bridge: "B",
    intro: "In",
    outro: "Out",
    tag: "T",
    ending: "E",
    refrain: "R",
    interlude: "Int",
  };
  const head = initials[key] ?? word.trim().slice(0, 1).toUpperCase() + word.trim().slice(1, 2);
  return `${head}${num}`;
}

/**
 * Put `text` back into `lyrics` in place of the slide at `index`.
 *
 * Slides are the lyrics split on blank lines, so editing one means rebuilding the
 * whole song. Doing it by index rather than by matching the old text matters: a
 * chorus appears verbatim several times, and replacing "the first block that looks
 * like this" would edit the wrong repeat.
 */
export function replaceBlock(lyrics: string, index: number, text: string): string {
  const blocks = lyrics.split(/\n\s*\n/);
  if (index < 0 || index >= blocks.length) return lyrics;
  return blocks.map((b, i) => (i === index ? text : b)).join("\n\n");
}

/** The block of `lyrics` a slide came from, for putting in an editor. */
export function blockAt(lyrics: string, index: number): string {
  const blocks = lyrics.split(/\n\s*\n/);
  return blocks[index] ?? "";
}
