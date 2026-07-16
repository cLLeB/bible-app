//! Learning on this machine, without ever losing what worked before.
//!
//! A speaker's tuning is three layers deep, and this module is what moves between them:
//!
//!   * **baked** — what the installer shipped for the President and Vice-President.
//!     Immutable, always restorable. It is the floor: no amount of local learning can
//!     take the church below the tuning they were given.
//!   * **previous** — whatever was in force before the last local change. Kept so an
//!     accepted change can be undone the moment it disappoints, without waiting for a
//!     new build or re-running the wizard.
//!   * **current** — what listening actually uses.
//!
//! Nothing here overwrites blindly. Applying a change stashes what it replaces first,
//! so "keep it" and "put it back the way it was" are both one step.

use crate::db::Db;
use crate::profile_seed::{apply_entry, baked_entry, capture, clear, SeedEntry};

/// Where the layer a change replaced is kept, per speaker.
fn previous_key(profile: &str) -> String {
    format!("previous:{profile}")
}

/// Is there a version to go back to for this speaker?
pub fn has_previous(db: &Db, profile: &str) -> bool {
    db.get_setting(&previous_key(profile)).is_some()
}

/// Can this speaker be reset to something the installer shipped?
pub fn has_baked(profile: &str) -> bool {
    baked_entry(profile).is_some()
}

/// Replace a speaker's tuning with `next`, keeping what it replaced so the operator
/// can put it back. The replacement is total, not a merge: an alias the app learned
/// wrongly must actually disappear when the operator undoes the change that added it.
pub fn install(db: &Db, profile: &str, next: &SeedEntry) -> rusqlite::Result<()> {
    let current = capture(db, profile);
    db.set_setting(&previous_key(profile), &serde_json::to_string(&current).unwrap_or_default())?;
    clear(db, profile)?;
    apply_entry(db, profile, next)
}

/// Put back the version in force before the last change. The layer being undone is
/// itself stashed, so an operator who rolls back and thinks better of it can roll
/// forward again — undo must not be a one-way door either.
pub fn rollback(db: &Db, profile: &str) -> rusqlite::Result<bool> {
    let Some(json) = db.get_setting(&previous_key(profile)) else { return Ok(false) };
    let Ok(previous) = serde_json::from_str::<SeedEntry>(&json) else { return Ok(false) };
    install(db, profile, &previous)?;
    Ok(true)
}

/// Strip every local layer and put back exactly what the installer shipped for this
/// speaker. The last resort when local learning has gone wrong in a way nobody wants
/// to unpick — and the reason a church can let the app learn at all without risk.
/// A speaker the build never knew about has no floor to return to.
pub fn reset_to_baked(db: &Db, profile: &str) -> rusqlite::Result<bool> {
    let Some(baked) = baked_entry(profile) else { return Ok(false) };
    install(db, profile, &baked)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stt::{Decode, Window};
    use std::path::Path;

    fn entry(alias: &str, beam: i32, room: f32) -> SeedEntry {
        let mut aliases = std::collections::BTreeMap::new();
        aliases.insert(alias.to_string(), "Neh".to_string());
        let mut decode = std::collections::BTreeMap::new();
        decode.insert(
            crate::flavor::model_file("base"),
            crate::profile_seed::DecodeSeed {
                beam,
                prompt: true,
                normalize: false,
                window: "full".into(),
            },
        );
        SeedEntry { aliases, room: Some(room), translation: Some("NKJV".into()), decode }
    }

    fn open() -> Db {
        let db = crate::db::open_at(Path::new(":memory:")).unwrap();
        db.migrate().unwrap();
        db
    }

    fn beam_of(db: &Db, profile: &str) -> i32 {
        crate::calibrate::load(db, Path::new(&crate::flavor::model_file("base")), profile).beam
    }

    #[test]
    fn installing_keeps_what_it_replaced_and_rollback_puts_it_back() {
        let db = open();
        apply_entry(&db, "President", &entry("hemaiah", 5, 0.01)).unwrap();
        assert!(!has_previous(&db, "President"), "nothing has been replaced yet");

        install(&db, "President", &entry("nemayer", 1, 0.02)).unwrap();
        assert_eq!(beam_of(&db, "President"), 1);
        assert!(has_previous(&db, "President"));

        assert!(rollback(&db, "President").unwrap());
        assert_eq!(beam_of(&db, "President"), 5, "the settings that worked are back");
        let names = crate::learn::book_names(&db, "President");
        assert_eq!(names.get("hemaiah").map(String::as_str), Some("Neh"));
        assert!(
            !names.contains_key("nemayer"),
            "an alias the undone version added must actually be gone, not merged"
        );

        // Rolling back is itself undoable: the operator can change their mind twice.
        assert!(rollback(&db, "President").unwrap());
        assert_eq!(beam_of(&db, "President"), 1);
    }

    #[test]
    fn a_speaker_with_no_previous_version_has_nothing_to_roll_back_to() {
        let db = open();
        assert!(!rollback(&db, "Guest").unwrap());
    }

    #[test]
    fn the_baked_profile_is_a_floor_local_learning_cannot_take_the_church_below() {
        let db = open();
        // Whatever this build ships for the President is what reset must restore.
        let Some(baked) = baked_entry("President") else {
            return; // a build with no baked seed has no floor to test
        };
        apply_entry(&db, "President", &baked).unwrap();

        // Local learning takes the profile somewhere else entirely.
        install(&db, "President", &entry("wrongly-learned", 9, 0.04)).unwrap();
        assert!(crate::learn::book_names(&db, "President").contains_key("wrongly-learned"));

        assert!(reset_to_baked(&db, "President").unwrap());
        assert_eq!(capture(&db, "President").aliases, baked.aliases, "back to the shipped names");
        assert!(!crate::learn::book_names(&db, "President").contains_key("wrongly-learned"));
    }

    #[test]
    fn a_guest_the_build_never_knew_has_no_floor_to_reset_to() {
        let db = open();
        apply_entry(&db, "Guest — Pastor Mensah", &entry("nemayer", 3, 0.02)).unwrap();
        assert!(!has_baked("Guest — Pastor Mensah"));
        assert!(!reset_to_baked(&db, "Guest — Pastor Mensah").unwrap());
        // And their own tuning is untouched by the attempt.
        assert_eq!(beam_of(&db, "Guest — Pastor Mensah"), 3);
    }

    #[test]
    fn capture_round_trips_a_live_profile_through_the_seed_shape() {
        let db = open();
        crate::calibrate::save(
            &db,
            Path::new(&crate::flavor::model_file("base")),
            "Vice-President",
            &Decode { beam: 2, prompt: false, normalize: true, window: Window::Fit { margin: 1.5 } },
        )
        .unwrap();
        crate::learn::save_book_name(&db, "Vice-President", "romins", "Rom").unwrap();
        crate::learn::save_translation(&db, "Vice-President", "KJV").unwrap();

        let snap = capture(&db, "Vice-President");
        clear(&db, "Vice-President").unwrap();
        assert!(crate::learn::book_names(&db, "Vice-President").is_empty());
        apply_entry(&db, "Vice-President", &snap).unwrap();

        assert_eq!(
            crate::calibrate::load(&db, Path::new(&crate::flavor::model_file("base")), "Vice-President"),
            Decode { beam: 2, prompt: false, normalize: true, window: Window::Fit { margin: 1.5 } }
        );
        assert_eq!(crate::learn::load_translation(&db, "Vice-President"), Some("KJV".into()));
        assert_eq!(
            crate::learn::book_names(&db, "Vice-President").get("romins").map(String::as_str),
            Some("Rom")
        );
    }
}
