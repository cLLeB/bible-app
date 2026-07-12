use crate::reference::ParsedRef;
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
CREATE VIRTUAL TABLE IF NOT EXISTS verses_fts
    USING fts5(text, content='verses', content_rowid='id');
CREATE TABLE IF NOT EXISTS songs (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    author TEXT,
    lyrics TEXT NOT NULL,
    built_in INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS song_slides (
    id INTEGER PRIMARY KEY,
    song_id INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
    order_index INTEGER NOT NULL,
    text TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_slides_song ON song_slides (song_id, order_index);
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

#[cfg_attr(not(test), allow(dead_code))] // used by unit tests
pub fn open_in_memory() -> rusqlite::Result<Db> {
    Ok(Db { conn: Connection::open_in_memory()? })
}

pub fn open_at(path: &Path) -> rusqlite::Result<Db> {
    Ok(Db { conn: Connection::open(path)? })
}

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

#[derive(Debug, Clone)]
pub struct VerseRecord {
    pub book_osis: String,
    pub chapter: u16,
    pub verse: u16,
    pub text: String,
    pub translation: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SongSummary {
    pub id: i64,
    pub title: String,
    pub author: Option<String>,
    pub built_in: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideRecord {
    pub order_index: u16,
    pub text: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SongDetail {
    pub id: i64,
    pub title: String,
    pub author: Option<String>,
    pub lyrics: String,
}

#[derive(serde::Deserialize)]
struct DefaultSong {
    title: String,
    author: Option<String>,
    lyrics: String,
}

impl Db {
    pub fn migrate(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(MIGRATION)?;
        // Add built_in to pre-existing songs tables (ignore if already present).
        let _ = self
            .conn
            .execute("ALTER TABLE songs ADD COLUMN built_in INTEGER NOT NULL DEFAULT 0", []);
        Ok(())
    }

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

    /// Rebuild the FTS index only when the verse count has changed since the
    /// last build (tracked in `settings`), so later launches are instant.
    pub fn sync_fts(&self) -> rusqlite::Result<()> {
        let verses: i64 = self.conn.query_row("SELECT count(*) FROM verses", [], |r| r.get(0))?;
        let stored: i64 = self
            .conn
            .query_row("SELECT value FROM settings WHERE key = 'fts_count'", [], |r| {
                r.get::<_, String>(0)
            })
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(-1);
        if stored != verses {
            self.conn
                .execute("INSERT INTO verses_fts(verses_fts) VALUES('rebuild')", [])?;
            self.conn.execute("DELETE FROM settings WHERE key = 'fts_count'", [])?;
            self.conn.execute(
                "INSERT INTO settings(key, value) VALUES('fts_count', ?1)",
                [verses.to_string()],
            )?;
        }
        Ok(())
    }

    /// Full-text search verses (BM25). Returns records best-first with their
    /// rank (lower = better match).
    pub fn search_fts(
        &self,
        translation_code: &str,
        query: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<(VerseRecord, f64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT v.book_osis, v.chapter, v.verse, v.text, t.code, bm25(verses_fts) AS rank
             FROM verses_fts
             JOIN verses v ON v.id = verses_fts.rowid
             JOIN translations t ON t.id = v.translation_id
             WHERE verses_fts MATCH ?1 AND t.code = ?2
             ORDER BY rank LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            (query, translation_code, limit as i64),
            |row| {
                Ok((
                    VerseRecord {
                        book_osis: row.get(0)?,
                        chapter: row.get(1)?,
                        verse: row.get(2)?,
                        text: row.get(3)?,
                        translation: row.get(4)?,
                    },
                    row.get::<_, f64>(5)?,
                ))
            },
        )?;
        rows.collect()
    }

    /// Fetch verses start..=end of a chapter and join their text. Returns None
    /// if the starting verse is absent.
    pub fn find_verse_range(
        &self,
        translation_code: &str,
        book_osis: &str,
        chapter: u16,
        start: u16,
        end: u16,
    ) -> rusqlite::Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT v.text FROM verses v JOIN translations t ON t.id = v.translation_id
             WHERE t.code = ?1 AND v.book_osis = ?2 AND v.chapter = ?3
               AND v.verse BETWEEN ?4 AND ?5 ORDER BY v.verse",
        )?;
        let rows = stmt.query_map((translation_code, book_osis, chapter, start, end), |r| {
            r.get::<_, String>(0)
        })?;
        let texts: Vec<String> = rows.collect::<rusqlite::Result<_>>()?;
        if texts.is_empty() {
            Ok(None)
        } else {
            Ok(Some(texts.join(" ")))
        }
    }

    /// Fetch one verse by exact coordinates.
    pub fn verse_at(
        &self,
        translation_code: &str,
        book_osis: &str,
        chapter: u16,
        verse: u16,
    ) -> rusqlite::Result<Option<VerseRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT v.book_osis, v.chapter, v.verse, v.text, t.code
             FROM verses v JOIN translations t ON t.id = v.translation_id
             WHERE t.code = ?1 AND v.book_osis = ?2 AND v.chapter = ?3 AND v.verse = ?4",
        )?;
        let mut rows = stmt.query((translation_code, book_osis, chapter, verse))?;
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

    /// Highest verse number present in a chapter (None if chapter absent).
    pub fn chapter_last_verse(
        &self,
        translation_code: &str,
        book_osis: &str,
        chapter: u16,
    ) -> rusqlite::Result<Option<u16>> {
        self.conn
            .query_row(
                "SELECT MAX(v.verse) FROM verses v JOIN translations t ON t.id = v.translation_id
                 WHERE t.code = ?1 AND v.book_osis = ?2 AND v.chapter = ?3",
                (translation_code, book_osis, chapter),
                |r| r.get::<_, Option<u16>>(0),
            )
    }

    /// Highest chapter number present in a book (None if book absent).
    pub fn book_last_chapter(
        &self,
        translation_code: &str,
        book_osis: &str,
    ) -> rusqlite::Result<Option<u16>> {
        self.conn.query_row(
            "SELECT MAX(v.chapter) FROM verses v JOIN translations t ON t.id = v.translation_id
             WHERE t.code = ?1 AND v.book_osis = ?2",
            (translation_code, book_osis),
            |r| r.get::<_, Option<u16>>(0),
        )
    }

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

    /// (Re)build the slide rows for a song from its lyrics.
    fn rebuild_slides(&self, song_id: i64, lyrics: &str) -> rusqlite::Result<()> {
        let slides = crate::slides::split_lyrics(lyrics);
        let tx = self.conn.unchecked_transaction()?;
        {
            tx.execute("DELETE FROM song_slides WHERE song_id = ?1", [song_id])?;
            let mut stmt = tx.prepare(
                "INSERT INTO song_slides (song_id, order_index, text) VALUES (?1, ?2, ?3)",
            )?;
            for (i, text) in slides.iter().enumerate() {
                stmt.execute((song_id, i as i64, text))?;
            }
        }
        tx.commit()
    }

    fn insert_song(
        &self,
        title: &str,
        author: Option<&str>,
        lyrics: &str,
        built_in: bool,
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO songs (title, author, lyrics, built_in) VALUES (?1, ?2, ?3, ?4)",
            (title, author, lyrics, built_in as i64),
        )?;
        let song_id = self.conn.last_insert_rowid();
        self.rebuild_slides(song_id, lyrics)?;
        Ok(song_id)
    }

    /// Insert a user song and its auto-split slides. Returns the new song id.
    pub fn add_song(&self, title: &str, author: Option<&str>, lyrics: &str) -> rusqlite::Result<i64> {
        self.insert_song(title, author, lyrics, false)
    }

    pub fn is_built_in(&self, id: i64) -> rusqlite::Result<bool> {
        let n: i64 = self
            .conn
            .query_row("SELECT built_in FROM songs WHERE id = ?1", [id], |r| r.get(0))
            .unwrap_or(0);
        Ok(n != 0)
    }

    /// Update a song's fields and re-split its slides.
    pub fn update_song(
        &self,
        id: i64,
        title: &str,
        author: Option<&str>,
        lyrics: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE songs SET title = ?2, author = ?3, lyrics = ?4 WHERE id = ?1",
            (id, title, author, lyrics),
        )?;
        self.rebuild_slides(id, lyrics)
    }

    pub fn delete_song(&self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM song_slides WHERE song_id = ?1", [id])?;
        self.conn.execute("DELETE FROM songs WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn all_songs_full(&self) -> rusqlite::Result<Vec<SongDetail>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, title, author, lyrics FROM songs ORDER BY title COLLATE NOCASE")?;
        let rows = stmt.query_map([], |r| {
            Ok(SongDetail { id: r.get(0)?, title: r.get(1)?, author: r.get(2)?, lyrics: r.get(3)? })
        })?;
        rows.collect()
    }

    pub fn get_song(&self, id: i64) -> rusqlite::Result<Option<SongDetail>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, title, author, lyrics FROM songs WHERE id = ?1")?;
        let mut rows = stmt.query([id])?;
        match rows.next()? {
            Some(row) => Ok(Some(SongDetail {
                id: row.get(0)?,
                title: row.get(1)?,
                author: row.get(2)?,
                lyrics: row.get(3)?,
            })),
            None => Ok(None),
        }
    }

    pub fn list_songs(&self) -> rusqlite::Result<Vec<SongSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, author, built_in FROM songs ORDER BY title COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(SongSummary {
                id: r.get(0)?,
                title: r.get(1)?,
                author: r.get(2)?,
                built_in: r.get::<_, i64>(3)? != 0,
            })
        })?;
        rows.collect()
    }

    pub fn has_translation(&self, code: &str) -> rusqlite::Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM translations WHERE code = ?1",
            [code],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    pub fn list_translations(&self) -> rusqlite::Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare("SELECT code, name FROM translations ORDER BY code")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }

    pub fn get_song_title(&self, song_id: i64) -> rusqlite::Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT title FROM songs WHERE id = ?1")?;
        let mut rows = stmt.query([song_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    fn song_exists(&self, title: &str) -> rusqlite::Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM songs WHERE title = ?1",
            [title],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Seed the bundled public-domain hymns once per `version`, adding only
    /// titles not already present (so user libraries are never disturbed).
    pub fn seed_default_songs(&self, json: &str, version: i64) -> rusqlite::Result<()> {
        let stored: i64 = self
            .conn
            .query_row("SELECT value FROM settings WHERE key = 'bundled_songs_v'", [], |r| {
                r.get::<_, String>(0)
            })
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if stored >= version {
            return Ok(());
        }
        let songs: Vec<DefaultSong> = serde_json::from_str(json)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        for s in &songs {
            if !self.song_exists(&s.title)? {
                self.insert_song(&s.title, s.author.as_deref(), &s.lyrics, true)?;
            }
        }
        self.conn.execute("DELETE FROM settings WHERE key = 'bundled_songs_v'", [])?;
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES('bundled_songs_v', ?1)",
            [version.to_string()],
        )?;
        Ok(())
    }

    pub fn get_song_slides(&self, song_id: i64) -> rusqlite::Result<Vec<SlideRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT order_index, text FROM song_slides WHERE song_id = ?1 ORDER BY order_index",
        )?;
        let rows = stmt.query_map([song_id], |r| {
            Ok(SlideRecord { order_index: r.get(0)?, text: r.get(1)? })
        })?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../../data/fixtures/web.sample.json");

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

    #[test]
    fn seed_inserts_verses_and_is_idempotent() {
        let db = open_in_memory().unwrap();
        db.migrate().unwrap();
        let n1 = db.seed_from_json(SAMPLE).unwrap();
        assert_eq!(n1, 3);
        let _ = db.seed_from_json(SAMPLE).unwrap();
        let total: i64 = db.conn.query_row("SELECT count(*) FROM verses", [], |r| r.get(0)).unwrap();
        assert_eq!(total, 3);
    }

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

    #[test]
    fn add_song_splits_and_lists() {
        let db = open_in_memory().unwrap();
        db.migrate().unwrap();
        let id = db.add_song("Amazing Grace", Some("John Newton"), "Verse 1 line\n\nVerse 2 line").unwrap();

        let slides = db.get_song_slides(id).unwrap();
        assert_eq!(slides.len(), 2);
        assert_eq!(slides[0].order_index, 0);
        assert_eq!(slides[1].text, "Verse 2 line");

        let songs = db.list_songs().unwrap();
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].title, "Amazing Grace");
        assert_eq!(db.get_song_title(id).unwrap().as_deref(), Some("Amazing Grace"));
    }

    #[test]
    fn fts_finds_quoted_verse() {
        let db = open_in_memory().unwrap();
        db.migrate().unwrap();
        db.seed_from_json(SAMPLE).unwrap();
        db.sync_fts().unwrap();
        let hits = db
            .search_fts("WEB", "\"loved\" OR \"world\" OR \"gave\" OR \"perish\"", 3)
            .unwrap();
        assert!(!hits.is_empty(), "expected an FTS hit");
        assert_eq!(hits[0].0.book_osis, "John");
        assert_eq!(hits[0].0.verse, 16);
    }

    #[test]
    fn update_and_delete_song() {
        let db = open_in_memory().unwrap();
        db.migrate().unwrap();
        let id = db.add_song("Old", None, "one block").unwrap();
        assert_eq!(db.get_song_slides(id).unwrap().len(), 1);

        db.update_song(id, "New Title", Some("Writer"), "A\n\nB\n\nC").unwrap();
        let detail = db.get_song(id).unwrap().unwrap();
        assert_eq!(detail.title, "New Title");
        assert_eq!(detail.author.as_deref(), Some("Writer"));
        assert_eq!(db.get_song_slides(id).unwrap().len(), 3);

        db.delete_song(id).unwrap();
        assert!(db.get_song(id).unwrap().is_none());
        assert_eq!(db.get_song_slides(id).unwrap().len(), 0);
    }
}
