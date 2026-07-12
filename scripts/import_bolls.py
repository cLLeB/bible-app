#!/usr/bin/env python3
"""Import a Bible translation from bolls.life into our canonical offline format.

This is a BUILD-TIME tool. It downloads a translation once (online), on your
machine, and writes data/<code>.canonical.json. The app seeds every
data/*.canonical.json into its bundled SQLite at first run, so the running app
never touches the network — translations are fully offline.

Source: bolls.life serves each translation as a single static JSON dump
    https://bolls.life/static/translations/<CODE>.json
which is an array of verse objects: {book (1..66), chapter, verse, text}.
Book numbering is canonical 1..66 (Genesis..Revelation), identical to our
books.rs BOOKS order, so mapping is a direct index.

LICENSING — bundle only PUBLIC-DOMAIN translations. Modern translations
(NIV, ESV, NKJV, NASB, NLT, MSG, AMP, CSB, NET, ...) are copyrighted and MUST
NOT be redistributed in a shipped app. This script refuses non-public-domain
codes unless you pass --force (use only for a translation you are licensed for).

Usage:
    python scripts/import_bolls.py KJV
    python scripts/import_bolls.py KJV ASV YLT GNV DRB
"""
import json
import re
import sys
import time
import urllib.request
import pathlib

BOLLS = "https://bolls.life/static/translations/{code}.json"

# OSIS ids in canonical order 1..66 — MUST match src-tauri/src/books.rs BOOKS.
OSIS_BY_ORDER = [
    "Gen", "Exod", "Lev", "Num", "Deut", "Josh", "Judg", "Ruth", "1Sam", "2Sam",
    "1Kgs", "2Kgs", "1Chr", "2Chr", "Ezra", "Neh", "Esth", "Job", "Ps", "Prov",
    "Eccl", "Song", "Isa", "Jer", "Lam", "Ezek", "Dan", "Hos", "Joel", "Amos",
    "Obad", "Jonah", "Mic", "Nah", "Hab", "Zeph", "Hag", "Zech", "Mal", "Matt",
    "Mark", "Luke", "John", "Acts", "Rom", "1Cor", "2Cor", "Gal", "Eph", "Phil",
    "Col", "1Thess", "2Thess", "1Tim", "2Tim", "Titus", "Phlm", "Heb", "Jas",
    "1Pet", "2Pet", "1John", "2John", "3John", "Jude", "Rev",
]

# Translations free to bundle and ship. Public-domain classics plus modern
# translations released under a free/unrestricted license (BSB, WEB). Modern
# COPYRIGHTED translations (NIV, NLT, ESV, NKJV, NASB, MSG, AMP, CSB, ...) are
# deliberately NOT here — they cannot be redistributed without a publisher
# license, regardless of bolls hosting them.
PUBLIC_DOMAIN = {
    "BSB": "Berean Standard Bible",          # modern English, free for any use
    "WEB": "World English Bible",            # modern English, public domain
    "KJV": "King James Version (1769)",
    "ASV": "American Standard Version (1901)",
    "YLT": "Young's Literal Translation (1898)",
    "GNV": "Geneva Bible (1599)",
    "DRB": "Douay-Rheims Bible",
    "DARBY": "Darby Translation (1890)",
    "BBE": "Bible in Basic English (1949)",
    "WBT": "Webster's Bible (1833)",
    "LXXE": "Brenton Septuagint (English, 1851)",
    "LSV": "Literal Standard Version",
}

# Copyrighted — only importable with --force (personal use only). Names for
# nicer display; presence here does NOT bypass the --force requirement.
LICENSED = {
    "NIV": "New International Version",
    "NLT": "New Living Translation",
    "ESV": "English Standard Version",
    "NKJV": "New King James Version",
    "NASB": "New American Standard Bible",
    "CSB17": "Christian Standard Bible",
    "AMP": "Amplified Bible",
    "MSG": "The Message",
    "NET": "New English Translation",
    "GNT": "Good News Bible",
    "GNTD": "Good News Translation",
    "RSV": "Revised Standard Version",
    "NRSVCE": "New Revised Standard Version",
    "CEB": "Common English Bible",
    "CEVD": "Contemporary English Version",
    "CJB": "Complete Jewish Bible",
    "TLV": "Tree of Life Version",
    "LSB": "Legacy Standard Bible",
    "MEV": "Modern English Version",
    "ISV": "International Standard Version",
    "ERV": "Easy-to-Read Version",
    "NLV": "New Life Version",
    "NABRE": "New American Bible",
}

# Strong's numbers, footnotes, notes, section headings: remove tag AND content.
DROP_WITH_CONTENT_RE = re.compile(r"<(S|f|n|h)>.*?</\1>", re.DOTALL | re.IGNORECASE)
# Line break, used both for section headings AND genuine poetry lines.
BR_RE = re.compile(r"<br\s*/?>", re.IGNORECASE)
# Any remaining tag (<J>, <i>, <e>, <pb/>, ...): drop the tag, keep the content.
TAG_RE = re.compile(r"<[^>]+>")


def _seg_text(seg: str) -> str:
    return TAG_RE.sub("", seg).strip()


def _is_heading(seg: str) -> bool:
    """A leading <br/>-separated segment that is an editorial section heading
    ("The Beginning", "The Sermon on the Mount") rather than verse text.

    Deliberately strict — losing a real verse is far worse than leaving a
    heading. A heading is: short (<=6 words), title-like (>=2 capitalized words
    or a bare number like "Psalm 23"), and — crucially — carries NO terminal
    punctuation. Real verses almost always end in . , ; : ! ? so this protects
    exclamations ("...O LORD!") and name lists that end mid-clause."""
    s = _seg_text(seg)
    if not s:
        return False
    words = s.split()
    if not (1 <= len(words) <= 6):
        return False
    if s[-1] in ".,;:!?—–":  # any terminal/dash punctuation → not a heading
        return False
    caps = sum(1 for w in words if w[:1].isupper())
    has_number = any(w.isdigit() for w in words)
    return caps >= 2 or has_number


def _strip_leading_headings(text: str) -> str:
    """Drop editorial section-heading segments at the start of a verse, keeping
    genuine poetry line breaks (which become spaces). A heading is only stripped
    when it is followed by a substantial (>=4-word) segment — so short name-list
    entries and single-line verses are never touched. Empty leading/trailing
    segments (from a stray <br/>) are dropped first."""
    segs = BR_RE.split(text)
    while segs and not _seg_text(segs[0]):
        segs.pop(0)
    while segs and not _seg_text(segs[-1]):
        segs.pop()
    i = 0
    while i + 1 < len(segs) and _is_heading(segs[i]) and len(_seg_text(segs[i + 1]).split()) >= 4:
        i += 1
    return " ".join(segs[i:])


def clean(text: str) -> str:
    """Strip MyBible/HTML markup; drop Strong's/footnotes/notes and leading
    section headings; keep words inside formatting tags; normalize whitespace."""
    t = DROP_WITH_CONTENT_RE.sub("", str(text))
    t = _strip_leading_headings(t)
    t = TAG_RE.sub(" ", t)
    return " ".join(t.split())


def fetch_json(url: str, attempts: int = 4):
    """GET + parse JSON, retrying on transient network errors (the dumps are
    several MB and the single-core server sometimes drops mid-stream)."""
    last = None
    for i in range(attempts):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "bible-app-importer"})
            with urllib.request.urlopen(req, timeout=120) as resp:
                return json.loads(resp.read().decode("utf-8"))
        except Exception as e:  # network/parse errors are all retryable here
            last = e
            wait = 2 * (i + 1)
            print(f"  retry {i + 1}/{attempts} after error: {e} (waiting {wait}s)", file=sys.stderr)
            time.sleep(wait)
    raise last


def import_one(code: str, force: bool) -> None:
    name = PUBLIC_DOMAIN.get(code)
    if name is None:
        if not force:
            print(
                f"REFUSING '{code}': not in the public-domain allowlist. Bundling a "
                f"copyrighted translation is not permitted. Re-run with --force only "
                f"if you are licensed for it / for personal use.",
                file=sys.stderr,
            )
            return
        name = LICENSED.get(code, code)  # forced: personal-use responsibility

    url = BOLLS.format(code=code)
    print(f"Downloading {url} ...")
    rows = fetch_json(url)

    out = []
    skipped_books = set()
    for row in rows:
        book_id = int(row["book"])
        if not (1 <= book_id <= 66):
            skipped_books.add(book_id)  # apocrypha / non-canonical — we hold 66 books
            continue
        text = clean(row["text"])
        if not text:
            continue
        out.append({
            "book_osis": OSIS_BY_ORDER[book_id - 1],
            "chapter": int(row["chapter"]),
            "verse": int(row["verse"]),
            "text": text,
        })

    if skipped_books:
        print(f"  (skipped non-canonical book ids: {sorted(skipped_books)})", file=sys.stderr)

    result = {"translation": {"code": code, "name": name}, "verses": out}
    dest = pathlib.Path("data") / f"{code.lower()}.canonical.json"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(json.dumps(result, ensure_ascii=False), encoding="utf-8")
    print(f"Wrote {len(out)} verses to {dest}")


def main(argv: list) -> None:
    force = "--force" in argv
    codes = [a for a in argv if not a.startswith("--")]
    if not codes:
        print(__doc__)
        print("Public-domain codes:", ", ".join(sorted(PUBLIC_DOMAIN)))
        return
    failed = []
    for code in codes:
        try:
            import_one(code.upper(), force)
        except Exception as e:  # keep going; report at the end
            print(f"FAILED {code}: {e}", file=sys.stderr)
            failed.append(code.upper())
    if failed:
        print(f"\nDone with failures: {failed}. Re-run those codes to retry.", file=sys.stderr)


if __name__ == "__main__":
    main(sys.argv[1:])
