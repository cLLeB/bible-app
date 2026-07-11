#!/usr/bin/env python3
"""Normalize a source WEB JSON export into data/web.canonical.json.

Canonical output shape:
{ "translation": {"code","name"}, "verses": [ {"book_osis","chapter","verse","text"} ] }

Confirm SOURCE_FIELDS + BOOK_NAME_TO_OSIS against your downloaded file, then run:
    python scripts/normalize_web.py path/to/source_web.json
"""
import json, sys, pathlib

# --- confirm these against your source file ---
SOURCE_FIELDS = {"book": "book_name", "chapter": "chapter", "verse": "verse", "text": "text"}
# Map source book display names -> our OSIS ids (must match books.rs BOOKS).
BOOK_NAME_TO_OSIS = {
    "Genesis": "Gen", "Exodus": "Exod", "Leviticus": "Lev", "Numbers": "Num",
    "Deuteronomy": "Deut", "Joshua": "Josh", "Judges": "Judg", "Ruth": "Ruth",
    "1 Samuel": "1Sam", "2 Samuel": "2Sam", "1 Kings": "1Kgs", "2 Kings": "2Kgs",
    "1 Chronicles": "1Chr", "2 Chronicles": "2Chr", "Ezra": "Ezra", "Nehemiah": "Neh",
    "Esther": "Esth", "Job": "Job", "Psalms": "Ps", "Proverbs": "Prov",
    "Ecclesiastes": "Eccl", "Song of Solomon": "Song", "Isaiah": "Isa", "Jeremiah": "Jer",
    "Lamentations": "Lam", "Ezekiel": "Ezek", "Daniel": "Dan", "Hosea": "Hos",
    "Joel": "Joel", "Amos": "Amos", "Obadiah": "Obad", "Jonah": "Jonah", "Micah": "Mic",
    "Nahum": "Nah", "Habakkuk": "Hab", "Zephaniah": "Zeph", "Haggai": "Hag",
    "Zechariah": "Zech", "Malachi": "Mal", "Matthew": "Matt", "Mark": "Mark",
    "Luke": "Luke", "John": "John", "Acts": "Acts", "Romans": "Rom",
    "1 Corinthians": "1Cor", "2 Corinthians": "2Cor", "Galatians": "Gal",
    "Ephesians": "Eph", "Philippians": "Phil", "Colossians": "Col",
    "1 Thessalonians": "1Thess", "2 Thessalonians": "2Thess", "1 Timothy": "1Tim",
    "2 Timothy": "2Tim", "Titus": "Titus", "Philemon": "Phlm", "Hebrews": "Heb",
    "James": "Jas", "1 Peter": "1Pet", "2 Peter": "2Pet", "1 John": "1John",
    "2 John": "2John", "3 John": "3John", "Jude": "Jude", "Revelation": "Rev",
}

def main(src_path: str) -> None:
    raw = json.loads(pathlib.Path(src_path).read_text(encoding="utf-8"))
    rows = raw if isinstance(raw, list) else raw.get("verses", raw.get("rows", []))
    out = []
    skipped = set()
    for row in rows:
        name = row[SOURCE_FIELDS["book"]]
        osis = BOOK_NAME_TO_OSIS.get(name)
        if osis is None:
            skipped.add(name)
            continue
        out.append({
            "book_osis": osis,
            "chapter": int(row[SOURCE_FIELDS["chapter"]]),
            "verse": int(row[SOURCE_FIELDS["verse"]]),
            "text": " ".join(str(row[SOURCE_FIELDS["text"]]).split()),
        })
    if skipped:
        print("WARNING unmapped book names:", sorted(skipped), file=sys.stderr)
    result = {"translation": {"code": "WEB", "name": "World English Bible"}, "verses": out}
    dest = pathlib.Path("data/web.canonical.json")
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(json.dumps(result, ensure_ascii=False), encoding="utf-8")
    print(f"Wrote {len(out)} verses to {dest}")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: python scripts/normalize_web.py <source_web.json>")
    main(sys.argv[1])
