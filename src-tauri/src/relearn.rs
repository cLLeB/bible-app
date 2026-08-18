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
use crate::profile_seed::{apply_entry, baked_entry, capture, clear, DecodeSeed, SeedEntry};
use serde::{Deserialize, Serialize};

/// Where the layer a change replaced is kept, per speaker.
fn previous_key(profile: &str) -> String {
    format!("previous:{profile}")
}

fn proposal_key(profile: &str) -> String {
    format!("proposal:{profile}")
}

/// What a learning pass would like to change about a speaker, and the evidence for it.
/// A proposal is never applied by the app itself: the operator reads the numbers and
/// decides. Until then it sits in the database changing nothing.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub profile: String,
    /// The tuning being proposed, ready to install as-is.
    pub next: SeedEntry,
    /// Which model's settings were scored (settings are per model).
    pub model_file: String,
    /// Plain-language name of the proposed settings, e.g. "beam 5 · full window".
    pub settings: String,
    /// Passages read aloud across the services learned from — the marks available.
    pub references: usize,
    /// How many of them the settings in force recover.
    pub incumbent_score: usize,
    /// How many the proposed settings recover.
    pub new_score: usize,
    /// Misheard book names this pass found that the speaker did not already have.
    pub new_aliases: Vec<String>,
    pub minutes: f32,
    /// The services it learned from, newest first.
    pub sessions: Vec<String>,
    /// When it was worked out (epoch ms as a string, like a session name).
    pub at: String,
}

/// A baked speaker's acoustic settings are not re-tuned from a single service. One
/// odd Sunday — a cold, a different hall, a failing cable — must not be allowed to
/// overwrite tuning derived from far more preaching than that.
pub const MIN_SERVICES_TO_RETUNE: usize = crate::sessions::MIN_KEEP;

/// Store what a pass came up with, for the operator to look at when they next open
/// the app. One proposal per speaker: a newer pass supersedes an older one.
pub fn save_proposal(db: &Db, p: &Proposal) -> rusqlite::Result<()> {
    db.set_setting(&proposal_key(&p.profile), &serde_json::to_string(p).unwrap_or_default())
}

pub fn load_proposal(db: &Db, profile: &str) -> Option<Proposal> {
    db.get_setting(&proposal_key(profile)).and_then(|j| serde_json::from_str(&j).ok())
}

pub fn drop_proposal(db: &Db, profile: &str) -> rusqlite::Result<()> {
    db.delete_setting(&proposal_key(profile))
}

/// Turn a finished learning pass into a proposal — or into nothing, when there is
/// nothing worth proposing.
///
/// Two rules keep local learning from making a church worse off:
///   * settings are only proposed when they beat the ones in force **on the same
///     audio**. Equal is not better: a change that buys nothing is not worth the
///     risk of being different.
///   * a speaker the build shipped tuned is not re-tuned acoustically until there
///     are enough services to justify it. Misheard names are additive and low-risk,
///     so they may still be proposed — those are what a single service is good for.
///
/// Returns `None` when the pass found nothing the speaker doesn't already have.
pub fn propose(
    db: &Db,
    profile: &str,
    model_file: &str,
    learned: &crate::learn::Learned,
    sessions: Vec<String>,
) -> Option<Proposal> {
    let current = capture(db, profile);
    let incumbent_score = learned.incumbent.unwrap_or(0);
    let settings_help = learned.after > incumbent_score;
    let enough_services = sessions.len() >= MIN_SERVICES_TO_RETUNE || !has_baked(profile);
    let retune = settings_help && enough_services;

    let new_aliases: Vec<String> = learned
        .aliases
        .iter()
        .filter(|(word, _)| !current.aliases.contains_key(word))
        .map(|(word, _)| word.clone())
        .collect();
    if !retune && new_aliases.is_empty() {
        return None; // nothing to tell the operator about
    }

    // Start from what the speaker has and change only what was earned. The aliases
    // are merged, not replaced: names learned on earlier Sundays are still true.
    let mut next = current.clone();
    for (word, osis) in &learned.aliases {
        next.aliases.entry(word.clone()).or_insert_with(|| osis.clone());
    }
    if retune {
        next.decode.insert(model_file.to_string(), DecodeSeed::from_decode(&learned.decode));
        next.room = Some(learned.room.speech_above);
        if let Some(code) = &learned.translation {
            next.translation = Some(code.clone());
        }
    }

    Some(Proposal {
        profile: profile.to_string(),
        next,
        model_file: model_file.to_string(),
        settings: if retune {
            crate::calibrate::label_of(&learned.decode)
        } else {
            "unchanged".to_string()
        },
        references: learned.references_found,
        incumbent_score,
        new_score: if retune { learned.after } else { incumbent_score },
        new_aliases,
        minutes: learned.minutes,
        sessions,
        at: crate::sessions::now_stamp(),
    })
}

/// Take a proposal up: install it (stashing what it replaces) and clear it away.
pub fn accept(db: &Db, profile: &str) -> rusqlite::Result<bool> {
    let Some(p) = load_proposal(db, profile) else { return Ok(false) };
    install(db, profile, &p.next)?;
    drop_proposal(db, profile)?;
    Ok(true)
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
            crate::flavor::model_file("small"),
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
        crate::calibrate::load(db, Path::new(&crate::flavor::model_file("small")), profile).beam
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

    fn learned(after: usize, incumbent: usize, aliases: &[(&str, &str)]) -> crate::learn::Learned {
        crate::learn::Learned {
            decode: Decode { beam: 8, prompt: true, normalize: true, window: Window::Full },
            room: crate::learn::Room { speech_above: 0.03 },
            translation: Some("NIV".into()),
            aliases: aliases.iter().map(|(w, o)| (w.to_string(), o.to_string())).collect(),
            references_found: 12,
            before: 6,
            after,
            incumbent: Some(incumbent),
            minutes: 42.0,
            recordings: 2,
        }
    }

    fn services(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("{:013}", 1_000_000 + i)).collect()
    }

    #[test]
    fn settings_are_only_proposed_when_they_beat_what_is_already_in_force() {
        let db = open();
        let model = crate::flavor::model_file("small");
        apply_entry(&db, "President", &entry("hemaiah", 5, 0.01)).unwrap();

        // Worse on the same audio: nothing to say.
        assert!(propose(&db, "President", &model, &learned(7, 9, &[]), services(3)).is_none());
        // Equal is not better — a change that buys nothing is not worth being different for.
        assert!(propose(&db, "President", &model, &learned(9, 9, &[]), services(3)).is_none());

        // Better, with enough services behind it: propose the new settings.
        let p = propose(&db, "President", &model, &learned(11, 9, &[]), services(3)).unwrap();
        assert_eq!((p.incumbent_score, p.new_score, p.references), (9, 11, 12));
        assert_eq!(p.next.decode.get(&model).unwrap().beam, 8);
        // And a proposal, even once stored, changes nothing until it is accepted.
        save_proposal(&db, &p).unwrap();
        assert_eq!(beam_of(&db, "President"), 5);

        assert!(accept(&db, "President").unwrap());
        assert_eq!(beam_of(&db, "President"), 8);
        assert!(load_proposal(&db, "President").is_none(), "an accepted proposal is spent");
        assert!(has_previous(&db, "President"), "and what it replaced is still recoverable");
    }

    #[test]
    fn one_service_never_retunes_a_speaker_the_build_shipped_tuned() {
        let db = open();
        let model = crate::flavor::model_file("small");
        apply_entry(&db, "President", &entry("hemaiah", 5, 0.01)).unwrap();

        // A single Sunday, even a flattering one, must not overwrite baked tuning...
        let one = propose(&db, "President", &model, &learned(12, 9, &[("nemayer", "Neh")]), services(1));
        let p = one.expect("but the misheard name it found is still worth having");
        assert_eq!(p.next.decode.get(&model).unwrap().beam, 5, "settings untouched");
        assert_eq!(p.settings, "unchanged");
        assert_eq!(p.new_score, p.incumbent_score, "no acoustic claim is made");
        assert_eq!(p.new_aliases, vec!["nemayer".to_string()]);
        assert_eq!(p.next.aliases.get("hemaiah").map(String::as_str), Some("Neh"), "kept");

        // ...and with nothing else to offer, that same Sunday says nothing at all.
        assert!(propose(&db, "President", &model, &learned(12, 9, &[]), services(1)).is_none());

        // Enough services, and the same evidence does re-tune.
        let p = propose(&db, "President", &model, &learned(12, 9, &[]), services(MIN_SERVICES_TO_RETUNE))
            .unwrap();
        assert_eq!(p.next.decode.get(&model).unwrap().beam, 8);
    }

    #[test]
    fn a_guest_can_be_tuned_from_their_very_first_service() {
        let db = open();
        let model = crate::flavor::model_file("small");
        // Nobody shipped a tuning for a guest, so there is nothing to protect — a
        // single sermon is all they have and it is better than nothing.
        let p = propose(&db, "Guest — Pastor Mensah", &model, &learned(10, 4, &[]), services(1)).unwrap();
        assert_eq!(p.next.decode.get(&model).unwrap().beam, 8);
    }

    #[test]
    fn capture_round_trips_a_live_profile_through_the_seed_shape() {
        let db = open();
        crate::calibrate::save(
            &db,
            Path::new(&crate::flavor::model_file("small")),
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
            crate::calibrate::load(&db, Path::new(&crate::flavor::model_file("small")), "Vice-President"),
            Decode { beam: 2, prompt: false, normalize: true, window: Window::Fit { margin: 1.5 } }
        );
        assert_eq!(crate::learn::load_translation(&db, "Vice-President"), Some("KJV".into()));
        assert_eq!(
            crate::learn::book_names(&db, "Vice-President").get("romins").map(String::as_str),
            Some("Rom")
        );
    }
}
