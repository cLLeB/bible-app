/**
 * Human-readable song export/import (plain text, not JSON). One song per block,
 * blocks separated by a line of "===":
 *
 *   Amazing Grace
 *   by John Newton
 *
 *   Amazing grace, how sweet the sound
 *   That saved a wretch like me
 *
 *   ===
 *
 *   Blessed Assurance
 *
 *   Blessed assurance, Jesus is mine
 */
export interface SongText {
  title: string;
  author: string | null;
  lyrics: string;
}

export function songsToText(songs: SongText[]): string {
  return songs
    .map((s) => `${s.title}${s.author ? `\nby ${s.author}` : ""}\n\n${s.lyrics.trim()}`)
    .join("\n\n===\n\n");
}

export function parseSongsText(text: string): SongText[] {
  return text
    .replace(/\r\n/g, "\n")
    .split(/^\s*={3,}\s*$/m)
    .map((chunk): SongText | null => {
      const lines = chunk.split("\n");
      while (lines.length && !lines[0].trim()) lines.shift();
      if (!lines.length) return null;
      const title = (lines.shift() as string).trim();
      let author: string | null = null;
      if (lines.length && /^by\s+/i.test(lines[0].trim())) {
        author = (lines.shift() as string).trim().replace(/^by\s+/i, "");
      }
      while (lines.length && !lines[0].trim()) lines.shift();
      while (lines.length && !lines[lines.length - 1].trim()) lines.pop();
      const lyrics = lines.join("\n");
      return title && lyrics ? { title, author, lyrics } : null;
    })
    .filter((s): s is SongText => s !== null);
}
