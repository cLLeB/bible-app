# Phase 1A — Core Seam Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Type a Bible reference (e.g. `John 3:16`) in the operator console and have that verse render full-screen on the second monitor, backed by the real World English Bible stored in SQLite.

**Architecture:** Tauri 2 desktop app. A Rust core owns a SQLite Bible database (`rusqlite`, bundled), a reference parser, and a book-canon module; it exposes Tauri commands. A React operator window drives lookups; a second Tauri webview window on the second monitor renders the projected verse and reacts to a Tauri event. No AI in this phase — this proves the DB schema, IPC, and projection seam that every later phase plugs into.

**Tech Stack:** Tauri 2, Rust (`rusqlite` bundled, `serde`, `serde_json`, `thiserror`), React + TypeScript + Vite + Tailwind + Zustand, Python 3 (build-time only, WEB normalization).

## Global Constraints

- **Fully offline at runtime** — no network calls in shipped code paths. Data acquisition/normalization is build-time only.
- **Platform:** Windows 11 primary. Requires MSVC build tools + WebView2 (preinstalled on Win11).
- **Immutability:** functions return new values; no in-place mutation of shared state (per house style).
- **Files focused:** target 200–400 lines/file, 800 hard max. One responsibility per file.
- **Bundled translation for this phase:** WEB (World English Bible), public domain.
- **Canonical reference type** (used across tasks, defined in Task 3): `ParsedRef { book_osis: String, chapter: u16, verse: Option<u16> }`.
- **Canonical verse payload** (defined in Task 7): `VersePayload { reference: String, book: String, chapter: u16, verse: u16, text: String, translation: String }`.
- **TDD:** every Rust logic task writes the failing test first. Scaffold/UI tasks verify by build+run.
- **Commit** at the end of every task.

---

## File Structure

```
bible-app/
├─ src/                          # React operator + projection windows
│  ├─ main.tsx                   # operator window entry
│  ├─ App.tsx                    # operator console
│  ├─ store.ts                   # Zustand store (lookup state)
│  ├─ api.ts                     # typed wrappers over Tauri invoke/listen
│  ├─ components/LookupBar.tsx   # reference input + Project button
│  ├─ components/ResultCard.tsx  # shows looked-up verse before projecting
│  ├─ projection.tsx             # projection window entry
│  └─ ProjectionView.tsx         # full-screen verse renderer
├─ projection.html               # second Vite entry (projection window)
├─ index.html                    # operator window entry
├─ vite.config.ts                # multi-page: index.html + projection.html
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs                 # thin bin entry → lib::run()
│  │  ├─ lib.rs                  # Tauri builder, command registration, setup
│  │  ├─ books.rs               # book canon: name/abbrev → CanonicalBook
│  │  ├─ reference.rs           # parse "John 3:16" → ParsedRef
│  │  ├─ db.rs                  # open/migrate/seed + find_verse repo
│  │  ├─ commands.rs            # #[tauri::command] fns
│  │  └─ events.rs              # serde payload structs
│  ├─ Cargo.toml
│  └─ tauri.conf.json
├─ scripts/
│  └─ normalize_web.py           # build-time: source WEB JSON → canonical shape
├─ data/
│  ├─ web.canonical.json         # normalized WEB (git-ignored; large)
│  └─ fixtures/web.sample.json   # tiny fixture for tests (committed)
└─ docs/…                        # existing specs/backlog
```

---

### Task 1: Scaffold Tauri 2 + React/TS/Vite/Tailwind/Zustand

**Files:**
- Create: whole `create-tauri-app` scaffold into existing repo, then `src-tauri/Cargo.toml`, `tailwind.config.js`, `src/store.ts`.

**Interfaces:**
- Consumes: nothing.
- Produces: a runnable Tauri dev app (operator window) that later tasks extend.

- [ ] **Step 1: Scaffold into the current directory**

Run (in `bible-app/`, which already contains `docs/` and `.git`):
```bash
npm create tauri-app@latest . -- --template react-ts --manager npm --yes
```
If it refuses due to non-empty dir, scaffold in a temp dir and copy `src/`, `src-tauri/`, `index.html`, `vite.config.ts`, `package.json`, `tsconfig.json` over — do not overwrite `docs/` or `.git`.

- [ ] **Step 2: Add frontend deps (Tailwind, Zustand)**

Run:
```bash
npm install zustand
npm install -D tailwindcss@^3 postcss autoprefixer
npx tailwindcss init -p
```

- [ ] **Step 3: Configure Tailwind**

`tailwind.config.js`:
```js
/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./projection.html", "./src/**/*.{ts,tsx}"],
  theme: { extend: {} },
  plugins: [],
};
```
Replace `src/App.css`/`src/styles.css` top with:
```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

- [ ] **Step 4: Add Rust deps**

In `src-tauri/Cargo.toml` under `[dependencies]`:
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
```

- [ ] **Step 5: Verify it builds and runs**

Run:
```bash
npm run tauri dev
```
Expected: the default Tauri window opens. Close it. Then:
```bash
cd src-tauri && cargo build
```
Expected: compiles clean (rusqlite bundled builds SQLite from source — first build is slow).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: scaffold Tauri 2 + React/TS/Vite/Tailwind/Zustand"
```

---

### Task 2: Book canon module

**Files:**
- Create: `src-tauri/src/books.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod books;`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `struct CanonicalBook { pub osis: &'static str, pub name: &'static str, pub order: u8 }`
  - `fn resolve_book(input: &str) -> Option<&'static CanonicalBook>` — maps a spoken/typed book token (case-insensitive, handles `1/2/3`, `First/Second/Third`, common abbreviations) to its canonical book.

- [ ] **Step 1: Write the failing test**

Add to bottom of `src-tauri/src/books.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_full_names_case_insensitively() {
        assert_eq!(resolve_book("John").unwrap().osis, "John");
        assert_eq!(resolve_book("  john ").unwrap().osis, "John");
        assert_eq!(resolve_book("PSALMS").unwrap().osis, "Ps");
    }

    #[test]
    fn resolves_numbered_books() {
        assert_eq!(resolve_book("1 Corinthians").unwrap().osis, "1Cor");
        assert_eq!(resolve_book("First Corinthians").unwrap().osis, "1Cor");
        assert_eq!(resolve_book("1 cor").unwrap().osis, "1Cor");
        assert_eq!(resolve_book("2 John").unwrap().osis, "2John");
    }

    #[test]
    fn resolves_common_abbreviations() {
        assert_eq!(resolve_book("Gen").unwrap().osis, "Gen");
        assert_eq!(resolve_book("Rom").unwrap().osis, "Rom");
        assert_eq!(resolve_book("Ps").unwrap().osis, "Ps");
    }

    #[test]
    fn rejects_unknown() {
        assert!(resolve_book("Hogwarts").is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test books::`
Expected: FAIL — `resolve_book`/`CanonicalBook` not found.

- [ ] **Step 3: Write minimal implementation**

Top of `src-tauri/src/books.rs`:
```rust
#[derive(Debug, Clone, Copy)]
pub struct CanonicalBook {
    pub osis: &'static str,
    pub name: &'static str,
    pub order: u8,
}

// 66 books, canonical order. (osis, display name, order)
pub static BOOKS: &[CanonicalBook] = &[
    CanonicalBook { osis: "Gen", name: "Genesis", order: 1 },
    CanonicalBook { osis: "Exod", name: "Exodus", order: 2 },
    CanonicalBook { osis: "Lev", name: "Leviticus", order: 3 },
    CanonicalBook { osis: "Num", name: "Numbers", order: 4 },
    CanonicalBook { osis: "Deut", name: "Deuteronomy", order: 5 },
    CanonicalBook { osis: "Josh", name: "Joshua", order: 6 },
    CanonicalBook { osis: "Judg", name: "Judges", order: 7 },
    CanonicalBook { osis: "Ruth", name: "Ruth", order: 8 },
    CanonicalBook { osis: "1Sam", name: "1 Samuel", order: 9 },
    CanonicalBook { osis: "2Sam", name: "2 Samuel", order: 10 },
    CanonicalBook { osis: "1Kgs", name: "1 Kings", order: 11 },
    CanonicalBook { osis: "2Kgs", name: "2 Kings", order: 12 },
    CanonicalBook { osis: "1Chr", name: "1 Chronicles", order: 13 },
    CanonicalBook { osis: "2Chr", name: "2 Chronicles", order: 14 },
    CanonicalBook { osis: "Ezra", name: "Ezra", order: 15 },
    CanonicalBook { osis: "Neh", name: "Nehemiah", order: 16 },
    CanonicalBook { osis: "Esth", name: "Esther", order: 17 },
    CanonicalBook { osis: "Job", name: "Job", order: 18 },
    CanonicalBook { osis: "Ps", name: "Psalms", order: 19 },
    CanonicalBook { osis: "Prov", name: "Proverbs", order: 20 },
    CanonicalBook { osis: "Eccl", name: "Ecclesiastes", order: 21 },
    CanonicalBook { osis: "Song", name: "Song of Solomon", order: 22 },
    CanonicalBook { osis: "Isa", name: "Isaiah", order: 23 },
    CanonicalBook { osis: "Jer", name: "Jeremiah", order: 24 },
    CanonicalBook { osis: "Lam", name: "Lamentations", order: 25 },
    CanonicalBook { osis: "Ezek", name: "Ezekiel", order: 26 },
    CanonicalBook { osis: "Dan", name: "Daniel", order: 27 },
    CanonicalBook { osis: "Hos", name: "Hosea", order: 28 },
    CanonicalBook { osis: "Joel", name: "Joel", order: 29 },
    CanonicalBook { osis: "Amos", name: "Amos", order: 30 },
    CanonicalBook { osis: "Obad", name: "Obadiah", order: 31 },
    CanonicalBook { osis: "Jonah", name: "Jonah", order: 32 },
    CanonicalBook { osis: "Mic", name: "Micah", order: 33 },
    CanonicalBook { osis: "Nah", name: "Nahum", order: 34 },
    CanonicalBook { osis: "Hab", name: "Habakkuk", order: 35 },
    CanonicalBook { osis: "Zeph", name: "Zephaniah", order: 36 },
    CanonicalBook { osis: "Hag", name: "Haggai", order: 37 },
    CanonicalBook { osis: "Zech", name: "Zechariah", order: 38 },
    CanonicalBook { osis: "Mal", name: "Malachi", order: 39 },
    CanonicalBook { osis: "Matt", name: "Matthew", order: 40 },
    CanonicalBook { osis: "Mark", name: "Mark", order: 41 },
    CanonicalBook { osis: "Luke", name: "Luke", order: 42 },
    CanonicalBook { osis: "John", name: "John", order: 43 },
    CanonicalBook { osis: "Acts", name: "Acts", order: 44 },
    CanonicalBook { osis: "Rom", name: "Romans", order: 45 },
    CanonicalBook { osis: "1Cor", name: "1 Corinthians", order: 46 },
    CanonicalBook { osis: "2Cor", name: "2 Corinthians", order: 47 },
    CanonicalBook { osis: "Gal", name: "Galatians", order: 48 },
    CanonicalBook { osis: "Eph", name: "Ephesians", order: 49 },
    CanonicalBook { osis: "Phil", name: "Philippians", order: 50 },
    CanonicalBook { osis: "Col", name: "Colossians", order: 51 },
    CanonicalBook { osis: "1Thess", name: "1 Thessalonians", order: 52 },
    CanonicalBook { osis: "2Thess", name: "2 Thessalonians", order: 53 },
    CanonicalBook { osis: "1Tim", name: "1 Timothy", order: 54 },
    CanonicalBook { osis: "2Tim", name: "2 Timothy", order: 55 },
    CanonicalBook { osis: "Titus", name: "Titus", order: 56 },
    CanonicalBook { osis: "Phlm", name: "Philemon", order: 57 },
    CanonicalBook { osis: "Heb", name: "Hebrews", order: 58 },
    CanonicalBook { osis: "Jas", name: "James", order: 59 },
    CanonicalBook { osis: "1Pet", name: "1 Peter", order: 60 },
    CanonicalBook { osis: "2Pet", name: "2 Peter", order: 61 },
    CanonicalBook { osis: "1John", name: "1 John", order: 62 },
    CanonicalBook { osis: "2John", name: "2 John", order: 63 },
    CanonicalBook { osis: "3John", name: "3 John", order: 64 },
    CanonicalBook { osis: "Jude", name: "Jude", order: 65 },
    CanonicalBook { osis: "Rev", name: "Revelation", order: 66 },
];

// Minimal alias map for Phase 1 (full alias_engine is Phase 2).
// Maps a normalized key → osis. Includes short forms; numbered books are
// normalized separately (see normalize_ordinal).
fn abbrev_to_osis(key: &str) -> Option<&'static str> {
    let m: &[(&str, &str)] = &[
        ("gen", "Gen"), ("genesis", "Gen"),
        ("exod", "Exod"), ("exodus", "Exod"), ("ex", "Exod"),
        ("lev", "Lev"), ("leviticus", "Lev"),
        ("num", "Num"), ("numbers", "Num"),
        ("deut", "Deut"), ("deuteronomy", "Deut"),
        ("josh", "Josh"), ("joshua", "Josh"),
        ("judg", "Judg"), ("judges", "Judg"),
        ("ruth", "Ruth"),
        ("ezra", "Ezra"), ("neh", "Neh"), ("nehemiah", "Neh"),
        ("esth", "Esth"), ("esther", "Esth"),
        ("job", "Job"),
        ("ps", "Ps"), ("psalm", "Ps"), ("psalms", "Ps"),
        ("prov", "Prov"), ("proverbs", "Prov"),
        ("eccl", "Eccl"), ("ecclesiastes", "Eccl"),
        ("song", "Song"), ("song of solomon", "Song"), ("songofsolomon", "Song"),
        ("isa", "Isa"), ("isaiah", "Isa"),
        ("jer", "Jer"), ("jeremiah", "Jer"),
        ("lam", "Lam"), ("lamentations", "Lam"),
        ("ezek", "Ezek"), ("ezekiel", "Ezek"),
        ("dan", "Dan"), ("daniel", "Dan"),
        ("hos", "Hos"), ("hosea", "Hos"),
        ("joel", "Joel"), ("amos", "Amos"),
        ("obad", "Obad"), ("obadiah", "Obad"),
        ("jonah", "Jonah"), ("mic", "Mic"), ("micah", "Mic"),
        ("nah", "Nah"), ("nahum", "Nah"),
        ("hab", "Hab"), ("habakkuk", "Hab"),
        ("zeph", "Zeph"), ("zephaniah", "Zeph"),
        ("hag", "Hag"), ("haggai", "Hag"),
        ("zech", "Zech"), ("zechariah", "Zech"),
        ("mal", "Mal"), ("malachi", "Mal"),
        ("matt", "Matt"), ("matthew", "Matt"), ("mt", "Matt"),
        ("mark", "Mark"), ("mk", "Mark"),
        ("luke", "Luke"), ("lk", "Luke"),
        ("john", "John"), ("jn", "John"),
        ("acts", "Acts"),
        ("rom", "Rom"), ("romans", "Rom"),
        ("gal", "Gal"), ("galatians", "Gal"),
        ("eph", "Eph"), ("ephesians", "Eph"),
        ("phil", "Phil"), ("philippians", "Phil"),
        ("col", "Col"), ("colossians", "Col"),
        ("titus", "Titus"),
        ("phlm", "Phlm"), ("philemon", "Phlm"),
        ("heb", "Heb"), ("hebrews", "Heb"),
        ("jas", "Jas"), ("james", "Jas"),
        ("jude", "Jude"),
        ("rev", "Rev"), ("revelation", "Rev"),
    ];
    m.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

// Turns leading "1/2/3", "i/ii/iii", "first/second/third" into a digit prefix.
// Returns (ordinal_digit, rest) e.g. "First Corinthians" -> (Some(1), "corinthians").
fn split_ordinal(norm: &str) -> (Option<u8>, String) {
    let words: Vec<&str> = norm.split_whitespace().collect();
    if let Some(first) = words.first() {
        let ord = match *first {
            "1" | "i" | "first" => Some(1),
            "2" | "ii" | "second" => Some(2),
            "3" | "iii" | "third" => Some(3),
            _ => None,
        };
        if ord.is_some() {
            return (ord, words[1..].join(" "));
        }
    }
    (None, norm.to_string())
}

// Stems for numbered books (used only when an ordinal prefix is present).
fn numbered_stem(key: &str) -> Option<&'static str> {
    let m: &[(&str, &str)] = &[
        ("sam", "Sam"), ("samuel", "Sam"),
        ("kgs", "Kgs"), ("kings", "Kgs"),
        ("chr", "Chr"), ("chronicles", "Chr"),
        ("cor", "Cor"), ("corinthians", "Cor"),
        ("thess", "Thess"), ("thessalonians", "Thess"),
        ("tim", "Tim"), ("timothy", "Tim"),
        ("pet", "Pet"), ("peter", "Pet"),
        ("john", "John"), ("jn", "John"),
    ];
    m.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// Look up a book directly by its OSIS id (used to render display names).
pub fn book_by_osis(osis: &str) -> Option<&'static CanonicalBook> {
    BOOKS.iter().find(|b| b.osis == osis)
}

pub fn resolve_book(input: &str) -> Option<&'static CanonicalBook> {
    let norm = input.trim().to_lowercase();
    let (ord, rest) = split_ordinal(&norm);
    let rest_key = rest.replace(' ', "");

    let target_osis: String = if let Some(n) = ord {
        // numbered book: ordinal + base stem (e.g. 1 + "Cor" -> "1Cor")
        let stem = numbered_stem(&rest).or_else(|| numbered_stem(&rest_key))?;
        format!("{n}{stem}")
    } else {
        abbrev_to_osis(&rest)
            .or_else(|| abbrev_to_osis(&rest_key))?
            .to_string()
    };
    book_by_osis(&target_osis)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test books::`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/books.rs src-tauri/src/lib.rs
git commit -m "feat(books): canonical book resolver (names, abbrevs, numbered books)"
```

---

### Task 3: Reference parser

**Files:**
- Create: `src-tauri/src/reference.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod reference;`)

**Interfaces:**
- Consumes: `books::resolve_book`.
- Produces:
  - `struct ParsedRef { pub book_osis: String, pub chapter: u16, pub verse: Option<u16> }`
  - `fn parse_reference(input: &str) -> Option<ParsedRef>` — parses `"John 3:16"`, `"Romans 8"`, `"1 Cor 13:4"`, `"Psalm 23"`, `"John 3 16"`.

- [ ] **Step 1: Write the failing test**

Bottom of `src-tauri/src/reference.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_book_chapter_verse() {
        let r = parse_reference("John 3:16").unwrap();
        assert_eq!(r.book_osis, "John");
        assert_eq!(r.chapter, 3);
        assert_eq!(r.verse, Some(16));
    }

    #[test]
    fn parses_chapter_only() {
        let r = parse_reference("Romans 8").unwrap();
        assert_eq!(r.book_osis, "Rom");
        assert_eq!(r.chapter, 8);
        assert_eq!(r.verse, None);
    }

    #[test]
    fn parses_numbered_book() {
        let r = parse_reference("1 Cor 13:4").unwrap();
        assert_eq!(r.book_osis, "1Cor");
        assert_eq!(r.chapter, 13);
        assert_eq!(r.verse, Some(4));
    }

    #[test]
    fn parses_space_separator() {
        let r = parse_reference("John 3 16").unwrap();
        assert_eq!(r.verse, Some(16));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_reference("hello world").is_none());
        assert!(parse_reference("John").is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test reference::`
Expected: FAIL — `parse_reference` not found.

- [ ] **Step 3: Write minimal implementation**

Top of `src-tauri/src/reference.rs`:
```rust
use crate::books::resolve_book;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ParsedRef {
    pub book_osis: String,
    pub chapter: u16,
    pub verse: Option<u16>,
}

pub fn parse_reference(input: &str) -> Option<ParsedRef> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Split trailing "<chapter>[:| ]<verse>?" from the book portion.
    // Strategy: find the last run that starts the numeric tail.
    let bytes: Vec<&str> = trimmed.split_whitespace().collect();

    // Walk from the end collecting number-ish tokens (may contain ':').
    let mut tail: Vec<&str> = Vec::new();
    let mut split_at = bytes.len();
    for (i, tok) in bytes.iter().enumerate().rev() {
        if tok.chars().all(|c| c.is_ascii_digit() || c == ':') && tok.chars().any(|c| c.is_ascii_digit()) {
            tail.insert(0, tok);
            split_at = i;
        } else {
            break;
        }
    }
    if tail.is_empty() || split_at == 0 {
        return None; // no numbers, or no book portion
    }

    let book_part = bytes[..split_at].join(" ");
    let book = resolve_book(&book_part)?;

    // Flatten tail into numbers. Accept "3:16", "3", "16" tokens or "3 16".
    let mut nums: Vec<u16> = Vec::new();
    for tok in tail {
        for piece in tok.split(':') {
            if piece.is_empty() {
                continue;
            }
            nums.push(piece.parse::<u16>().ok()?);
        }
    }
    let chapter = *nums.first()?;
    let verse = nums.get(1).copied();

    Some(ParsedRef {
        book_osis: book.osis.to_string(),
        chapter,
        verse,
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test reference::`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/reference.rs src-tauri/src/lib.rs
git commit -m "feat(reference): parse book/chapter/verse strings into ParsedRef"
```

---

### Task 4: SQLite schema + open/migrate

**Files:**
- Create: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod db;`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `struct Db { conn: rusqlite::Connection }`
  - `fn open_in_memory() -> rusqlite::Result<Db>` (tests)
  - `fn open_at(path: &std::path::Path) -> rusqlite::Result<Db>` (runtime)
  - `Db::migrate(&self) -> rusqlite::Result<()>` — creates `translations`, `books`, `verses` tables (subset of spec §5.3 sufficient for Phase 1).

- [ ] **Step 1: Write the failing test**

Bottom of `src-tauri/src/db.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_creates_tables() {
        let db = open_in_memory().unwrap();
        db.migrate().unwrap();
        let count: i64 = db
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('translations','books','verses')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test db::`
Expected: FAIL — `open_in_memory`/`migrate` not found.

- [ ] **Step 3: Write minimal implementation**

Top of `src-tauri/src/db.rs`:
```rust
use rusqlite::Connection;
use std::path::Path;

pub struct Db {
    pub conn: Connection,
}

const MIGRATION: &str = r#"
CREATE TABLE IF NOT EXISTS translations (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    is_public_domain INTEGER NOT NULL DEFAULT 1,
    is_bundled INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS books (
    id INTEGER PRIMARY KEY,
    osis TEXT NOT NULL,
    name TEXT NOT NULL,
    ord INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS verses (
    id INTEGER PRIMARY KEY,
    translation_id INTEGER NOT NULL REFERENCES translations(id),
    book_osis TEXT NOT NULL,
    chapter INTEGER NOT NULL,
    verse INTEGER NOT NULL,
    text TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_verse_lookup
    ON verses (translation_id, book_osis, chapter, verse);
"#;

pub fn open_in_memory() -> rusqlite::Result<Db> {
    Ok(Db { conn: Connection::open_in_memory()? })
}

pub fn open_at(path: &Path) -> rusqlite::Result<Db> {
    Ok(Db { conn: Connection::open(path)? })
}

impl Db {
    pub fn migrate(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(MIGRATION)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test db::`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db.rs src-tauri/src/lib.rs
git commit -m "feat(db): SQLite open + migration for translations/books/verses"
```

---

### Task 5: Seed verses from canonical JSON + acquire/normalize WEB

**Files:**
- Create: `scripts/normalize_web.py`, `data/fixtures/web.sample.json`
- Modify: `src-tauri/src/db.rs` (add `seed_from_json`), `.gitignore` (ignore `data/web.canonical.json`)

**Interfaces:**
- Consumes: `Db` from Task 4.
- Produces:
  - Canonical JSON shape: `{ "translation": {"code","name"}, "verses": [ {"book_osis","chapter","verse","text"} ] }`
  - `Db::seed_from_json(&self, json: &str) -> rusqlite::Result<usize>` — inserts translation + verses, returns verse count. Idempotent via `INSERT OR IGNORE`.

- [ ] **Step 1: Create the test fixture**

`data/fixtures/web.sample.json`:
```json
{
  "translation": { "code": "WEB", "name": "World English Bible" },
  "verses": [
    { "book_osis": "John", "chapter": 3, "verse": 16, "text": "For God so loved the world, that he gave his one and only Son, that whoever believes in him should not perish, but have eternal life." },
    { "book_osis": "Ps", "chapter": 23, "verse": 1, "text": "Yahweh is my shepherd; I shall lack nothing." },
    { "book_osis": "Rom", "chapter": 8, "verse": 28, "text": "We know that all things work together for good for those who love God, for those who are called according to his purpose." }
  ]
}
```

- [ ] **Step 2: Write the failing test**

Add to `db.rs` `tests` module:
```rust
    const SAMPLE: &str = include_str!("../../data/fixtures/web.sample.json");

    #[test]
    fn seed_inserts_verses_and_is_idempotent() {
        let db = open_in_memory().unwrap();
        db.migrate().unwrap();
        let n1 = db.seed_from_json(SAMPLE).unwrap();
        assert_eq!(n1, 3);
        // second run must not duplicate
        let _ = db.seed_from_json(SAMPLE).unwrap();
        let total: i64 = db.conn.query_row("SELECT count(*) FROM verses", [], |r| r.get(0)).unwrap();
        assert_eq!(total, 3);
    }
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cd src-tauri && cargo test db::tests::seed`
Expected: FAIL — `seed_from_json` not found.

- [ ] **Step 4: Implement `seed_from_json`**

Add to `db.rs`:
```rust
#[derive(serde::Deserialize)]
struct SeedFile {
    translation: SeedTranslation,
    verses: Vec<SeedVerse>,
}
#[derive(serde::Deserialize)]
struct SeedTranslation {
    code: String,
    name: String,
}
#[derive(serde::Deserialize)]
struct SeedVerse {
    book_osis: String,
    chapter: u16,
    verse: u16,
    text: String,
}

impl Db {
    pub fn seed_from_json(&self, json: &str) -> rusqlite::Result<usize> {
        let parsed: SeedFile = serde_json::from_str(json)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        self.conn.execute(
            "INSERT OR IGNORE INTO translations (code, name, is_public_domain, is_bundled) VALUES (?1, ?2, 1, 1)",
            (&parsed.translation.code, &parsed.translation.name),
        )?;
        let translation_id: i64 = self.conn.query_row(
            "SELECT id FROM translations WHERE code = ?1",
            [&parsed.translation.code],
            |r| r.get(0),
        )?;

        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO verses (translation_id, book_osis, chapter, verse, text)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for v in &parsed.verses {
                stmt.execute((translation_id, &v.book_osis, v.chapter, v.verse, &v.text))?;
            }
        }
        tx.commit()?;
        Ok(parsed.verses.len())
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd src-tauri && cargo test db::`
Expected: PASS (2 tests).

- [ ] **Step 6: Write the build-time WEB normalizer**

`scripts/normalize_web.py` — converts a source WEB JSON into our canonical shape. **The one thing to confirm against your actual download is the field mapping in `SOURCE_FIELDS`.** Recommended public-domain source: ebible.org WEB JSON, or the `getbible` v2 WEB export.
```python
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
```

- [ ] **Step 7: Ignore the large generated file**

Append to `.gitignore`:
```
data/web.canonical.json
```

- [ ] **Step 8: Generate the real data and sanity-check**

Download a public-domain WEB JSON, then run:
```bash
python scripts/normalize_web.py path/to/source_web.json
```
Expected: `Wrote 31102 verses to data/web.canonical.json` (count may vary slightly by source; no unmapped-book warnings). If book names are unmapped, adjust `BOOK_NAME_TO_OSIS`.

- [ ] **Step 9: Commit**

```bash
git add scripts/normalize_web.py data/fixtures/web.sample.json src-tauri/src/db.rs .gitignore
git commit -m "feat(db): JSON verse seeding + build-time WEB normalizer"
```

---

### Task 6: `find_verse` repository method

**Files:**
- Modify: `src-tauri/src/db.rs`

**Interfaces:**
- Consumes: `Db`, `reference::ParsedRef`.
- Produces:
  - `struct VerseRecord { pub book_osis: String, pub chapter: u16, pub verse: u16, pub text: String, pub translation: String }`
  - `Db::find_verse(&self, translation_code: &str, r: &ParsedRef) -> rusqlite::Result<Option<VerseRecord>>` — when `r.verse` is `None`, returns verse 1 of the chapter.

- [ ] **Step 1: Write the failing test**

Add to `db.rs` `tests`:
```rust
    use crate::reference::ParsedRef;

    #[test]
    fn find_verse_returns_exact_and_defaults_to_v1() {
        let db = open_in_memory().unwrap();
        db.migrate().unwrap();
        db.seed_from_json(SAMPLE).unwrap();

        let exact = db.find_verse("WEB", &ParsedRef { book_osis: "John".into(), chapter: 3, verse: Some(16) }).unwrap().unwrap();
        assert_eq!(exact.verse, 16);
        assert!(exact.text.starts_with("For God so loved"));
        assert_eq!(exact.translation, "WEB");

        let default_v1 = db.find_verse("WEB", &ParsedRef { book_osis: "Ps".into(), chapter: 23, verse: None }).unwrap().unwrap();
        assert_eq!(default_v1.verse, 1);

        let missing = db.find_verse("WEB", &ParsedRef { book_osis: "John".into(), chapter: 99, verse: Some(1) }).unwrap();
        assert!(missing.is_none());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test db::tests::find_verse`
Expected: FAIL — `find_verse`/`VerseRecord` not found.

- [ ] **Step 3: Implement**

Add to `db.rs`:
```rust
use crate::reference::ParsedRef;

#[derive(Debug, Clone)]
pub struct VerseRecord {
    pub book_osis: String,
    pub chapter: u16,
    pub verse: u16,
    pub text: String,
    pub translation: String,
}

impl Db {
    pub fn find_verse(
        &self,
        translation_code: &str,
        r: &ParsedRef,
    ) -> rusqlite::Result<Option<VerseRecord>> {
        let verse = r.verse.unwrap_or(1);
        let mut stmt = self.conn.prepare(
            "SELECT v.book_osis, v.chapter, v.verse, v.text, t.code
             FROM verses v JOIN translations t ON t.id = v.translation_id
             WHERE t.code = ?1 AND v.book_osis = ?2 AND v.chapter = ?3 AND v.verse = ?4",
        )?;
        let mut rows = stmt.query((translation_code, &r.book_osis, r.chapter, verse))?;
        if let Some(row) = rows.next()? {
            Ok(Some(VerseRecord {
                book_osis: row.get(0)?,
                chapter: row.get(1)?,
                verse: row.get(2)?,
                text: row.get(3)?,
                translation: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test db::`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db.rs
git commit -m "feat(db): find_verse repo method with chapter-default-to-v1"
```

---

### Task 7: Tauri commands + app state + events

**Files:**
- Create: `src-tauri/src/events.rs`, `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs` (managed state, DB init, register handlers)

**Interfaces:**
- Consumes: `books`, `reference::parse_reference`, `db::{Db, find_verse}`.
- Produces (callable from the frontend):
  - `VersePayload { reference, book, chapter, verse, text, translation }` (serde, camelCase).
  - `#[tauri::command] lookup_reference(query: String) -> Result<VersePayload, String>`
  - `#[tauri::command] project_verse(app, payload: VersePayload) -> Result<(), String>` — emits `set-projection` to the `projection` window (opening it if needed).
  - `#[tauri::command] blank_projection(app) -> Result<(), String>` — emits `set-projection` with `null`.

- [ ] **Step 1: Define the payload with a unit test**

`src-tauri/src/events.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VersePayload {
    pub reference: String,
    pub book: String,
    pub chapter: u16,
    pub verse: u16,
    pub text: String,
    pub translation: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serializes_camel_case() {
        let p = VersePayload {
            reference: "John 3:16".into(), book: "John".into(), chapter: 3,
            verse: 16, text: "For God...".into(), translation: "WEB".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"reference\":\"John 3:16\""));
        assert!(json.contains("\"translation\":\"WEB\""));
    }
}
```

- [ ] **Step 2: Run the payload test**

Run: `cd src-tauri && cargo test events::`
Expected: PASS (1 test).

- [ ] **Step 3: Implement commands**

`src-tauri/src/commands.rs`:
```rust
use crate::books::book_by_osis;
use crate::db::Db;
use crate::events::VersePayload;
use crate::reference::parse_reference;
use std::sync::Mutex;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub struct AppState {
    pub db: Mutex<Db>,
    pub translation: String, // active translation code, e.g. "WEB"
}

fn build_payload(rec: crate::db::VerseRecord) -> VersePayload {
    let book_name = book_by_osis(&rec.book_osis)
        .map(|b| b.name.to_string())
        .unwrap_or_else(|| rec.book_osis.clone());
    let reference = format!("{} {}:{}", book_name, rec.chapter, rec.verse);
    VersePayload {
        reference,
        book: book_name,
        chapter: rec.chapter,
        verse: rec.verse,
        text: rec.text,
        translation: rec.translation,
    }
}

#[tauri::command]
pub fn lookup_reference(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<VersePayload, String> {
    let parsed = parse_reference(&query).ok_or_else(|| format!("Could not parse '{query}'"))?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let rec = db
        .find_verse(&state.translation, &parsed)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Verse not found: '{query}'"))?;
    Ok(build_payload(rec))
}

fn ensure_projection_window(app: &tauri::AppHandle) -> Result<(), String> {
    if app.get_webview_window("projection").is_none() {
        WebviewWindowBuilder::new(app, "projection", WebviewUrl::App("projection.html".into()))
            .title("Projection")
            .decorations(false)
            .build()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn project_verse(app: tauri::AppHandle, payload: VersePayload) -> Result<(), String> {
    ensure_projection_window(&app)?;
    app.emit_to("projection", "set-projection", Some(payload))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn blank_projection(app: tauri::AppHandle) -> Result<(), String> {
    ensure_projection_window(&app)?;
    app.emit_to("projection", "set-projection", Option::<VersePayload>::None)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Wire state + handlers in `lib.rs`**

Ensure `src-tauri/src/lib.rs` contains (module declarations at top, then `run`):
```rust
mod books;
mod commands;
mod db;
mod events;
mod reference;

use commands::AppState;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // resolve app data dir; open + migrate + seed on first run
            let data_dir = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&data_dir).ok();
            let db_path = data_dir.join("bible.sqlite");
            let db = db::open_at(&db_path).expect("open db");
            db.migrate().expect("migrate");

            // Seed WEB once (idempotent). data/web.canonical.json is bundled as a resource.
            let seed_path = app
                .path()
                .resolve("web.canonical.json", tauri::path::BaseDirectory::Resource)
                .expect("resource path");
            if let Ok(json) = std::fs::read_to_string(&seed_path) {
                db.seed_from_json(&json).expect("seed");
            }

            app.manage(AppState { db: Mutex::new(db), translation: "WEB".into() });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::lookup_reference,
            commands::project_verse,
            commands::blank_projection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 5: Bundle the data file as a Tauri resource**

In `src-tauri/tauri.conf.json`, under `bundle`, add:
```json
"resources": ["../data/web.canonical.json"]
```
And confirm `src-tauri/capabilities/default.json` allows the core `event` + `window` permissions (default capability includes them in Tauri 2 templates; if not, add `"core:event:default"`, `"core:window:default"`, `"core:webview:default"`).

- [ ] **Step 6: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: compiles. (Runtime seeding needs Task 5's `web.canonical.json` present; if absent, the app still builds and runs with an empty Bible.)

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/events.rs src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/tauri.conf.json
git commit -m "feat(ipc): lookup_reference + project/blank commands, DB init + WEB seed on setup"
```

---

### Task 8: Operator UI (lookup + project)

**Files:**
- Create: `src/api.ts`, `src/store.ts`, `src/components/LookupBar.tsx`, `src/components/ResultCard.tsx`
- Modify: `src/App.tsx`, `index.html` (title)

**Interfaces:**
- Consumes: Tauri commands `lookup_reference`, `project_verse`, `blank_projection`.
- Produces: operator console that looks up a reference and projects it.

- [ ] **Step 1: Typed Tauri API wrappers**

`src/api.ts`:
```ts
import { invoke } from "@tauri-apps/api/core";

export interface VersePayload {
  reference: string;
  book: string;
  chapter: number;
  verse: number;
  text: string;
  translation: string;
}

export const lookupReference = (query: string) =>
  invoke<VersePayload>("lookup_reference", { query });

export const projectVerse = (payload: VersePayload) =>
  invoke<void>("project_verse", { payload });

export const blankProjection = () => invoke<void>("blank_projection");
```

- [ ] **Step 2: Zustand store**

`src/store.ts`:
```ts
import { create } from "zustand";
import type { VersePayload } from "./api";

interface LookupState {
  query: string;
  result: VersePayload | null;
  error: string | null;
  setQuery: (q: string) => void;
  setResult: (r: VersePayload | null) => void;
  setError: (e: string | null) => void;
}

export const useLookupStore = create<LookupState>((set) => ({
  query: "",
  result: null,
  error: null,
  setQuery: (query) => set({ query }),
  setResult: (result) => set({ result, error: null }),
  setError: (error) => set({ error, result: null }),
}));
```

- [ ] **Step 3: LookupBar**

`src/components/LookupBar.tsx`:
```tsx
import { FormEvent } from "react";
import { lookupReference } from "../api";
import { useLookupStore } from "../store";

export function LookupBar() {
  const { query, setQuery, setResult, setError } = useLookupStore();

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    try {
      const v = await lookupReference(query.trim());
      setResult(v);
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <form onSubmit={onSubmit} className="flex gap-2">
      <input
        autoFocus
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="e.g. John 3:16"
        className="flex-1 rounded border px-3 py-2 text-lg"
      />
      <button type="submit" className="rounded bg-blue-600 px-4 py-2 text-white">
        Look up
      </button>
    </form>
  );
}
```

- [ ] **Step 4: ResultCard**

`src/components/ResultCard.tsx`:
```tsx
import { blankProjection, projectVerse } from "../api";
import { useLookupStore } from "../store";

export function ResultCard() {
  const { result, error } = useLookupStore();
  if (error) return <p className="text-red-600">{error}</p>;
  if (!result) return null;
  return (
    <div className="rounded border p-4">
      <div className="mb-1 text-sm text-gray-500">
        {result.reference} · {result.translation}
      </div>
      <p className="mb-3 text-lg">{result.text}</p>
      <div className="flex gap-2">
        <button
          onClick={() => projectVerse(result)}
          className="rounded bg-green-600 px-4 py-2 text-white"
        >
          Project
        </button>
        <button onClick={() => blankProjection()} className="rounded border px-4 py-2">
          Blank
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 5: App shell**

`src/App.tsx`:
```tsx
import { LookupBar } from "./components/LookupBar";
import { ResultCard } from "./components/ResultCard";

export default function App() {
  return (
    <main className="mx-auto max-w-xl space-y-4 p-6">
      <h1 className="text-2xl font-bold">Bible — Operator Console</h1>
      <LookupBar />
      <ResultCard />
    </main>
  );
}
```

- [ ] **Step 6: Verify the operator flow (data required)**

Ensure `data/web.canonical.json` exists (Task 5 Step 8). Run:
```bash
npm run tauri dev
```
Type `John 3:16`, click **Look up** → verse text appears in the ResultCard. (Projection verified in Task 9.)

- [ ] **Step 7: Commit**

```bash
git add src/api.ts src/store.ts src/components/LookupBar.tsx src/components/ResultCard.tsx src/App.tsx index.html
git commit -m "feat(ui): operator console with reference lookup"
```

---

### Task 9: Projection window (second monitor)

**Files:**
- Create: `projection.html`, `src/projection.tsx`, `src/ProjectionView.tsx`
- Modify: `vite.config.ts` (multi-page input), `src-tauri/src/commands.rs` (position on 2nd monitor)

**Interfaces:**
- Consumes: Tauri event `set-projection` (payload `VersePayload | null`).
- Produces: a full-screen verse renderer on the second monitor.

- [ ] **Step 1: Second Vite entry point**

`projection.html` (repo root):
```html
<!doctype html>
<html lang="en">
  <head><meta charset="UTF-8" /><title>Projection</title></head>
  <body class="bg-black"><div id="root"></div><script type="module" src="/src/projection.tsx"></script></body>
</html>
```

- [ ] **Step 2: Configure Vite multi-page**

`vite.config.ts` — add `build.rollupOptions.input`:
```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        projection: resolve(__dirname, "projection.html"),
      },
    },
  },
});
```

- [ ] **Step 3: Projection entry + view**

`src/projection.tsx`:
```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { ProjectionView } from "./ProjectionView";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ProjectionView />
  </React.StrictMode>
);
```

`src/ProjectionView.tsx`:
```tsx
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { VersePayload } from "./api";

export function ProjectionView() {
  const [verse, setVerse] = useState<VersePayload | null>(null);

  useEffect(() => {
    const un = listen<VersePayload | null>("set-projection", (e) => setVerse(e.payload));
    return () => { un.then((f) => f()); };
  }, []);

  return (
    <div className="flex h-screen w-screen flex-col items-center justify-center bg-black px-16 text-center text-white">
      {verse ? (
        <>
          <p className="mb-8 max-w-5xl text-5xl leading-tight">{verse.text}</p>
          <p className="text-2xl text-gray-300">
            {verse.reference} · {verse.translation}
          </p>
        </>
      ) : null}
    </div>
  );
}
```

- [ ] **Step 4: Position the projection window on the second monitor**

Replace `ensure_projection_window` in `commands.rs` with:
```rust
fn ensure_projection_window(app: &tauri::AppHandle) -> Result<(), String> {
    if app.get_webview_window("projection").is_some() {
        return Ok(());
    }
    let win = WebviewWindowBuilder::new(app, "projection", WebviewUrl::App("projection.html".into()))
        .title("Projection")
        .decorations(false)
        .build()
        .map_err(|e| e.to_string())?;

    // Move to the second monitor if present, else stay on primary.
    if let Ok(monitors) = win.available_monitors() {
        if let Some(second) = monitors.get(1) {
            let pos = second.position();
            win.set_position(tauri::PhysicalPosition { x: pos.x, y: pos.y })
                .map_err(|e| e.to_string())?;
        }
    }
    win.set_fullscreen(true).map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 5: End-to-end verify**

Run: `npm run tauri dev`
With a second monitor connected: type `John 3:16` → **Look up** → **Project**. Expected: verse fills the second monitor on black; **Blank** clears it. With only one monitor: the projection window opens fullscreen on the primary display (acceptable for dev).

- [ ] **Step 6: Commit**

```bash
git add projection.html src/projection.tsx src/ProjectionView.tsx vite.config.ts src-tauri/src/commands.rs
git commit -m "feat(projection): second-monitor fullscreen verse window via set-projection event"
```

---

### Task 10: Full-slice verification + README

**Files:**
- Create: `README.md`

**Interfaces:** none (verification + docs).

- [ ] **Step 1: Run the whole Rust test suite**

Run: `cd src-tauri && cargo test`
Expected: all tests PASS (books 4, reference 5, db 3, events 1).

- [ ] **Step 2: Cold-start smoke test**

Delete the dev DB so seeding runs fresh (Windows path):
```bash
rm -f "$APPDATA/com.bible-app.app/bible.sqlite" 2>/dev/null || true
npm run tauri dev
```
Verify: `John 3:16`, `Psalm 23`, `Romans 8:28` each look up and project.

- [ ] **Step 3: Write a short README**

`README.md`:
```markdown
# Bible App (offline)

Phase 1A: manual reference lookup → second-monitor projection, WEB in SQLite.

## Dev setup
1. Install Rust, Node 18+, and (Windows) MSVC Build Tools + WebView2.
2. `npm install`
3. Provide Bible data: download a public-domain WEB JSON, then
   `python scripts/normalize_web.py <source_web.json>` (writes `data/web.canonical.json`).
4. `npm run tauri dev`

## Tests
- Rust: `cd src-tauri && cargo test`

See `docs/superpowers/specs/2026-07-11-offline-bible-app-design.md` for the frozen architecture.
```

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: Phase 1A README + verification notes"
```

---

## Self-Review Notes

- **Spec coverage (Phase 1 core seam):** Tauri shell (T1) ✓ · SQLite schema §5.3 subset (T4) ✓ · WEB load (T5) ✓ · manual reference lookup (T2,T3,T6,T7) ✓ · second-monitor projection + `ProjectionState` verse/blank subset (T7,T9) ✓. Songs, and the `events`/`config`/`pipeline`/`diagnostics` module skeletons, are **Phase 1B** (separate plan) — deliberately out of this slice.
- **Deferred correctly (not gaps):** no ML, no VAD/STT, no embeddings — those are Phases 2–3.
- **Type consistency:** `ParsedRef` (T3) consumed unchanged in T6/T7; `VerseRecord` (T6) → `build_payload` → `VersePayload` (T7) consumed by frontend `api.ts` (T8) and `ProjectionView` (T9) with identical camelCase fields.
- **Known simplification:** Task 2's numbered-book handling covers `1/2/3`, `First/Second/Third`, `i/ii/iii`; full alias_engine is Phase 2 per spec §5.7.
