#!/usr/bin/env python3
"""Build the Personal-tier song library from a plain-text file you provide.

This is for the operator's OWN songs (modern/copyrighted worship & gospel), used
privately in Personal / dev builds only. The output is gitignored and never
distributed — same boundary as the Personal-tier Bible translations.

You supply the lyrics (from your CCLI SongSelect account or wherever you hold the
rights). This script contains NO lyrics; it only formats what you give it.

INPUT  (default: data/personal-songs.txt) — one song per block, blocks split by a
line of "===":

    Way Maker
    by Sinach

    <lyrics line 1>
    <lyrics line 2>

    ===

    Waymaker (English)
    by Joe Mettle

    <lyrics>

OUTPUT: data/personal.songs.json  (seeded automatically on the next
`npm run tauri dev` / Personal build; re-run this whenever you edit the text).

Usage:
    python scripts/import_songs.py                 # reads data/personal-songs.txt
    python scripts/import_songs.py path/to/file.txt
"""
import json
import re
import sys
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_SRC = ROOT / "data" / "personal-songs.txt"
OUT = ROOT / "data" / "personal.songs.json"


def parse(text: str) -> list:
    songs = []
    for chunk in re.split(r"^\s*={3,}\s*$", text.replace("\r\n", "\n"), flags=re.MULTILINE):
        lines = chunk.split("\n")
        while lines and not lines[0].strip():
            lines.pop(0)
        if not lines:
            continue
        title = lines.pop(0).strip()
        author = None
        if lines and re.match(r"^by\s+", lines[0].strip(), re.IGNORECASE):
            author = re.sub(r"^by\s+", "", lines.pop(0).strip(), flags=re.IGNORECASE)
        while lines and not lines[0].strip():
            lines.pop(0)
        while lines and not lines[-1].strip():
            lines.pop()
        lyrics = "\n".join(lines).strip()
        if title and lyrics:
            songs.append({"title": title, "author": author, "lyrics": lyrics})
    return songs


def main(argv: list) -> None:
    src = pathlib.Path(argv[0]) if argv else DEFAULT_SRC
    if not src.exists():
        print(
            f"No source file at {src}\n\n"
            f"Create it and paste your songs (one per block, split by a line of '==='):\n\n"
            f"    Song Title\n    by Author\n\n    <lyrics>\n\n    ===\n\n    Next Song\n    ...\n",
            file=sys.stderr,
        )
        sys.exit(1)
    songs = parse(src.read_text(encoding="utf-8"))
    OUT.write_text(json.dumps(songs, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"Wrote {len(songs)} songs to {OUT} (gitignored; seeds on next Personal/dev run).")


if __name__ == "__main__":
    main(sys.argv[1:])
