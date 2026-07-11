/**
 * Slide-splitting helpers (mirror of the Rust `slides::split_lyrics` so the
 * editor can preview slides live without a round-trip). A blank line separates
 * slides; whitespace is trimmed and empty blocks dropped.
 */
export function splitLyrics(lyrics: string): string[] {
  return lyrics
    .replace(/\r\n/g, "\n")
    .split(/\n\s*\n/)
    .map((block) => block.trim())
    .filter((block) => block.length > 0);
}

/**
 * Smart-paste transform: re-group lyrics into slides of `n` non-empty lines
 * each, inserting blank lines so the standard blank-line split yields those
 * slides. Lets an operator paste raw lyrics and format them in one click.
 */
export function groupEveryNLines(lyrics: string, n: number): string {
  const lines = lyrics
    .replace(/\r\n/g, "\n")
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0);

  const groups: string[] = [];
  for (let i = 0; i < lines.length; i += n) {
    groups.push(lines.slice(i, i + n).join("\n"));
  }
  return groups.join("\n\n");
}
