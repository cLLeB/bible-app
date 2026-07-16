//! Ship the personal build already knowing its own church's preachers.
//!
//! `learn_cli` derives, per speaker, everything that is not constant about them —
//! decoder settings per model, the book names they get misheard as, their sound
//! desk's loudness, the version they read from — and writes it to a seed file that
//! is baked into the personal-tier installer. This applies that seed to a fresh
//! database on first launch, so the app arrives tuned for the president and the
//! vice-president without anyone running the learning wizard on the church laptop.
//!
//! Two guards keep it honest:
//!   * personal tier only (caller checks `flavor::is_personal`) — a distribution
//!     build, meant for other churches, must not carry this one's preachers.
//!   * once only (a `profiles_seeded` marker) — so a church's own re-calibration is
//!     never overwritten by the shipped defaults on the next launch.

use crate::db::Db;
use crate::stt::{Decode, Window};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Seed {
    /// Which speaker the app should start on — the regular preacher.
    #[serde(default)]
    pub active: Option<String>,
    /// One entry per speaker, keyed by profile name.
    #[serde(default)]
    pub entries: BTreeMap<String, SeedEntry>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct SeedEntry {
    /// Misheard word -> OSIS book id, learned from this speaker's own recordings.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    /// Speech threshold measured from their audio.
    #[serde(default)]
    pub room: Option<f32>,
    /// The version their readings matched.
    #[serde(default)]
    pub translation: Option<String>,
    /// Winning decode settings, keyed by whisper model filename (e.g.
    /// `ggml-base.en.bin`), so one seed serves every model flavor.
    #[serde(default)]
    pub decode: BTreeMap<String, DecodeSeed>,
}

/// A `Decode` in the plain shape the seed file stores it in.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DecodeSeed {
    pub beam: i32,
    pub prompt: bool,
    pub normalize: bool,
    /// `"full"` or a fit margin like `"1.5"`.
    pub window: String,
}

impl DecodeSeed {
    pub fn from_decode(d: &Decode) -> Self {
        let window = match d.window {
            Window::Full => "full".to_string(),
            Window::Fit { margin } => margin.to_string(),
        };
        Self { beam: d.beam, prompt: d.prompt, normalize: d.normalize, window }
    }

    pub fn to_decode(&self) -> Decode {
        let window = if self.window.eq_ignore_ascii_case("full") {
            Window::Full
        } else {
            self.window.parse::<f32>().map(|margin| Window::Fit { margin }).unwrap_or(Window::Full)
        };
        Decode { beam: self.beam, prompt: self.prompt, normalize: self.normalize, window }
    }
}

/// Apply the baked seed to `db`, unless it has been seeded before or a church has
/// already started calibrating. Returns whether anything was written. Never errors
/// the caller out of starting: a malformed or empty seed just does nothing.
pub fn apply(db: &Db, json: &str) -> rusqlite::Result<bool> {
    if db.get_setting("profiles_seeded").is_some() {
        return Ok(false); // already seeded, or the operator has their own settings
    }
    let seed: Seed = match serde_json::from_str(json) {
        Ok(s) => s,
        Err(_) => return Ok(false),
    };
    if seed.entries.is_empty() {
        // Nothing to seed yet — leave the marker unset so a later, populated build
        // can still seed on its first run.
        return Ok(false);
    }

    for (name, entry) in &seed.entries {
        for (heard, osis) in &entry.aliases {
            crate::learn::save_book_name(db, name, heard, osis)?;
        }
        if let Some(room) = entry.room {
            crate::learn::save_room(db, name, &crate::learn::Room { speech_above: room })?;
        }
        if let Some(code) = &entry.translation {
            crate::learn::save_translation(db, name, code)?;
        }
        for (model_file, d) in &entry.decode {
            crate::calibrate::save(db, Path::new(model_file), name, &d.to_decode())?;
        }
    }

    // Make the seeded speakers selectable: the default roster, plus any extra name
    // the seed introduces, in a stable order.
    let mut names: Vec<String> =
        crate::calibrate::DEFAULT_PROFILES.iter().map(|s| s.to_string()).collect();
    for key in seed.entries.keys() {
        if !names.contains(key) {
            names.push(key.clone());
        }
    }
    db.set_setting("voice_profiles", &serde_json::to_string(&names).unwrap_or_default())?;
    if let Some(active) = &seed.active {
        db.set_setting("voice_profile", active)?;
    }

    db.set_setting("profiles_seeded", "1")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    const JSON: &str = r#"{
      "active": "Miss Hilda",
      "entries": {
        "Miss Hilda": {
          "aliases": {"hemaiah": "Neh"},
          "room": 0.012,
          "translation": "NKJV",
          "decode": {
            "ggml-base.en.bin":  {"beam": 5, "prompt": true, "normalize": false, "window": "full"},
            "ggml-small.en.bin": {"beam": 1, "prompt": true, "normalize": true,  "window": "1.5"}
          }
        },
        "Vice-President": {
          "aliases": {},
          "room": 0.02,
          "translation": null,
          "decode": {
            "ggml-base.en.bin": {"beam": 5, "prompt": true, "normalize": true, "window": "1.2"}
          }
        }
      }
    }"#;

    #[test]
    fn seeds_both_speakers_then_reads_back_per_model() {
        let db = db::open_at(Path::new(":memory:")).unwrap();
        db.migrate().unwrap();
        assert!(apply(&db, JSON).unwrap());

        // Decode round-trips, and the two models are kept apart.
        assert_eq!(
            crate::calibrate::load(&db, Path::new("ggml-base.en.bin"), "Miss Hilda"),
            Decode { beam: 5, prompt: true, normalize: false, window: Window::Full }
        );
        assert_eq!(
            crate::calibrate::load(&db, Path::new("ggml-small.en.bin"), "Miss Hilda"),
            Decode { beam: 1, prompt: true, normalize: true, window: Window::Fit { margin: 1.5 } }
        );
        assert_eq!(
            crate::calibrate::load(&db, Path::new("ggml-base.en.bin"), "Vice-President"),
            Decode { beam: 5, prompt: true, normalize: true, window: Window::Fit { margin: 1.2 } }
        );

        // Room, translation and misheard names all land against the right speaker.
        assert!((crate::learn::load_room(&db, "Miss Hilda").speech_above - 0.012).abs() < 1e-6);
        assert_eq!(crate::learn::load_translation(&db, "Miss Hilda"), Some("NKJV".into()));
        assert_eq!(crate::learn::load_translation(&db, "Vice-President"), None);
        assert_eq!(
            crate::learn::book_names(&db, "Miss Hilda").get("hemaiah").map(String::as_str),
            Some("Neh")
        );

        // Both speakers are selectable and the regular preacher is active.
        assert_eq!(crate::calibrate::active_profile(&db), "Miss Hilda");
        let profiles = crate::calibrate::profiles(&db);
        assert!(profiles.contains(&"Miss Hilda".to_string()));
        assert!(profiles.contains(&"Vice-President".to_string()));
    }

    #[test]
    fn seeding_is_a_once_only_act() {
        let db = db::open_at(Path::new(":memory:")).unwrap();
        db.migrate().unwrap();
        assert!(apply(&db, JSON).unwrap());
        // A church that recalibrates one speaker must not be reset by the next launch.
        crate::calibrate::save(
            &db,
            Path::new("ggml-base.en.bin"),
            "Miss Hilda",
            &Decode { beam: 1, prompt: false, normalize: false, window: Window::Full },
        )
        .unwrap();
        assert!(!apply(&db, JSON).unwrap());
        assert_eq!(
            crate::calibrate::load(&db, Path::new("ggml-base.en.bin"), "Miss Hilda"),
            Decode { beam: 1, prompt: false, normalize: false, window: Window::Full }
        );
    }

    #[test]
    fn an_empty_seed_writes_nothing_and_stays_unsealed() {
        let db = db::open_at(Path::new(":memory:")).unwrap();
        db.migrate().unwrap();
        assert!(!apply(&db, "{}").unwrap());
        assert!(db.get_setting("profiles_seeded").is_none());
    }
}
