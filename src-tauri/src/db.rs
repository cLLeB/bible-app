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
CREATE TABLE IF NOT EXISTS songs (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    author TEXT,
    lyrics TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS song_slides (
    id INTEGER PRIMARY KEY,
    song_id INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
    order_index INTEGER NOT NULL,
    text TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_slides_song ON song_slides (song_id, order_index);
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
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideRecord {
    pub order_index: u16,
    pub text: String,
}

impl Db {
    pub fn migrate(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(MIGRATION)
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

    /// Insert a song and its auto-split slides. Returns the new song id.
    pub fn add_song(&self, title: &str, author: Option<&str>, lyrics: &str) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO songs (title, author, lyrics) VALUES (?1, ?2, ?3)",
            (title, author, lyrics),
        )?;
        let song_id = self.conn.last_insert_rowid();

        let slides = crate::slides::split_lyrics(lyrics);
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO song_slides (song_id, order_index, text) VALUES (?1, ?2, ?3)",
            )?;
            for (i, text) in slides.iter().enumerate() {
                stmt.execute((song_id, i as i64, text))?;
            }
        }
        tx.commit()?;
        Ok(song_id)
    }

    pub fn list_songs(&self) -> rusqlite::Result<Vec<SongSummary>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, title, author FROM songs ORDER BY title COLLATE NOCASE")?;
        let rows = stmt.query_map([], |r| {
            Ok(SongSummary { id: r.get(0)?, title: r.get(1)?, author: r.get(2)? })
        })?;
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
}
