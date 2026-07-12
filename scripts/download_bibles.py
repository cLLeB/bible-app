#!/usr/bin/env python3
"""Download public-domain Bible translations from getbible v2 and normalize
them into data/<CODE>.canonical.json (our seed format).

Run: python scripts/download_bibles.py
Requires internet (build-time only). Files land in data/ and are loaded offline.
"""
import json, sys, time, urllib.request, pathlib

# Canonical book order — must match src-tauri/src/books.rs BOOKS.
OSIS = [
    "Gen","Exod","Lev","Num","Deut","Josh","Judg","Ruth","1Sam","2Sam","1Kgs","2Kgs",
    "1Chr","2Chr","Ezra","Neh","Esth","Job","Ps","Prov","Eccl","Song","Isa","Jer","Lam",
    "Ezek","Dan","Hos","Joel","Amos","Obad","Jonah","Mic","Nah","Hab","Zeph","Hag","Zech",
    "Mal","Matt","Mark","Luke","John","Acts","Rom","1Cor","2Cor","Gal","Eph","Phil","Col",
    "1Thess","2Thess","1Tim","2Tim","Titus","Phlm","Heb","Jas","1Pet","2Pet","1John","2John",
    "3John","Jude","Rev",
]

# getbible abbreviation -> our translation code (public-domain English set)
TRANSLATIONS = {
    "kjv": "KJV",
    "web": "WEB",
    "asv": "ASV",
    "ylt": "YLT",
    "basicenglish": "BBE",
    "darby": "DARBY",
}

UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120 Safari/537.36"

def fetch_book(code, nr, retries=4):
    url = f"https://api.getbible.net/v2/{code}/{nr}.json"
    for attempt in range(retries):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "application/json"})
            with urllib.request.urlopen(req, timeout=40) as r:
                return json.loads(r.read().decode("utf-8"))
        except Exception as e:
            if attempt == retries - 1:
                raise
            time.sleep(1.5 * (attempt + 1))

def build(code):
    verses = []
    name = None
    for nr in range(1, 67):
        data = fetch_book(code, nr)
        if name is None:
            name = data.get("translation", code.upper())
        osis = OSIS[nr - 1]
        for ch in data["chapters"]:
            for v in ch["verses"]:
                verses.append({
                    "book_osis": osis,
                    "chapter": int(v["chapter"]),
                    "verse": int(v["verse"]),
                    "text": " ".join(str(v["text"]).split()),
                })
        time.sleep(0.05)
    return name or code.upper(), verses

def main():
    dest_dir = pathlib.Path("data")
    dest_dir.mkdir(exist_ok=True)
    for gb_code, our_code in TRANSLATIONS.items():
        try:
            print(f"Downloading {our_code} ({gb_code}) …", flush=True)
            name, verses = build(gb_code)
            out = {"translation": {"code": our_code, "name": name}, "verses": verses}
            dest = dest_dir / f"{our_code}.canonical.json"
            dest.write_text(json.dumps(out, ensure_ascii=False), encoding="utf-8")
            print(f"  wrote {len(verses)} verses -> {dest}", flush=True)
        except Exception as e:
            print(f"  FAILED {our_code}: {e}", file=sys.stderr, flush=True)

if __name__ == "__main__":
    main()
