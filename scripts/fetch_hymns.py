#!/usr/bin/env python3
"""Fetch verbatim public-domain hymn lyrics and build src-tauri/default-songs.json
(bundled with the app, seeded on first run). Source: marvinjude/gospel-hymns
(classic hymns, all pre-1929 = public domain in the US).

Run: python scripts/fetch_hymns.py
"""
import json, re, sys, time, urllib.parse, urllib.request, pathlib

BASE = "https://raw.githubusercontent.com/marvinjude/gospel-hymns/master/Hymns/"
UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120 Safari/537.36"

# Curated, clearly public-domain hymns (file name in the repo).
FILES = [
    "11 Holy Holy Holy.txt", "13 To God Be The Glory.txt",
    "130 Just As I Am Without One Plea.txt", "169 What A Friend We Have In Jesus.txt",
    "17 Nothing But The Blood.txt", "170 Sweet Hour Of Prayer.txt",
    "18 Blessed Assurance.txt", "211 Crown Him With Many Crowns.txt",
    "214 All Hail The Power Of Jesus.txt", "227 Nearer My God To Thee.txt",
    "24 It Is Well With My Soul.txt", "243 Standing On The Promises.txt",
    "43 Trust And Obey.txt", "72 The Solid Rock.txt", "8 Blessed Be The Name.txt",
    "82 When I Survey The Wondrous Cross.txt", "127 Rock Of Ages Cleft For Me.txt",
    "10 Oh For A Thousand.txt", "111 Rescue The Perishing.txt",
    "138 When We All Get To Heaven.txt", "15 O Worship The King.txt",
    "165 Guide Me O Thou Great Jehovah.txt", "168 I Need Thee Every Hour.txt",
    "171 Pass Me Not.txt", "201 Love Lifted Me.txt", "39 Power In The Blood.txt",
    "48 Tis So Sweet To Trust.txt", "50 Count Your Blessings.txt",
    "80 At The Cross.txt", "92 I Surrender All.txt",
]

def title_of(fname: str) -> str:
    name = re.sub(r"^\d+\s+", "", fname).rsplit(".txt", 1)[0]
    return name.strip()

def clean(text: str) -> str:
    text = text.replace("\r\n", "\n").strip()
    text = re.sub(r"\n{3,}", "\n\n", text)  # collapse extra blank lines
    lines = [ln.rstrip() for ln in text.split("\n")]
    return "\n".join(lines).strip()

def fetch(fname: str) -> str:
    url = BASE + urllib.parse.quote(fname)
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=30) as r:
        return r.read().decode("utf-8")

def main() -> None:
    songs = []
    for fname in FILES:
        try:
            lyrics = clean(fetch(fname))
            if len(lyrics) < 40:
                print(f"  skip (too short): {fname}", file=sys.stderr)
                continue
            songs.append({"title": title_of(fname), "author": None, "lyrics": lyrics})
            print(f"  ok: {title_of(fname)}")
            time.sleep(0.05)
        except Exception as e:
            print(f"  FAILED {fname}: {e}", file=sys.stderr)
    dest = pathlib.Path("src-tauri/default-songs.json")
    dest.write_text(json.dumps(songs, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"Wrote {len(songs)} hymns -> {dest}")

if __name__ == "__main__":
    main()
