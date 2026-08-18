//! Build flavor — baked in at compile time by the build script (scripts/
//! build_flavors.py) via environment variables. Two things vary per flavor:
//!
//!   * TIER: `personal` allows copyrighted translations (for the user's OWN,
//!     non-distributed use); `distribution` is public-domain only.
//!   * MODELS: which whisper model(s) this flavor ships with.
//!
//! Defaults (no env set) = the TESTING flavor: Personal tier + all models, so a
//! plain `cargo`/`tauri` dev build can exercise everything at once.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Personal,
    Distribution,
}

pub fn tier() -> Tier {
    match option_env!("BIBLE_APP_TIER") {
        Some("distribution") => Tier::Distribution,
        _ => Tier::Personal,
    }
}

/// Personal tier may download/bundle copyrighted translations for private use.
pub fn is_personal() -> bool {
    tier() == Tier::Personal
}

pub fn tier_name() -> &'static str {
    match tier() {
        Tier::Personal => "personal",
        Tier::Distribution => "distribution",
    }
}

/// The whisper models this flavor exposes (first is the default). Testing/dev
/// ships all of them.
pub fn models() -> Vec<&'static str> {
    match option_env!("BIBLE_APP_MODELS") {
        Some(s) if !s.trim().is_empty() => {
            s.split(',').map(|m| m.trim()).filter(|m| !m.is_empty()).collect()
        }
        // No flavor set, so this is `npm run tauri dev` or a plain `cargo run`.
        // `small` leads deliberately: the first entry is what `default_model` hands
        // out, and a dev build that quietly transcribes with a weaker model than any
        // shipped flavor is a trap. It cost a testing session that judged accuracy on
        // `base` while the installed build ran `small`, produced transcripts like
        // "John Torey, Sistine", and looked for all the world like a regression.
        //
        // Speed is no longer the reason to prefer base either: on a graphics card
        // small runs an utterance in about 1.4s here, which is faster than base
        // managed on the processor.
        _ => vec!["small", "medium"],
    }
}

pub fn default_model() -> &'static str {
    models().first().copied().unwrap_or("small")
}

/// The whisper file a model kind ("base") is kept under. Settings are stored per
/// model file, so anything reasoning about a speaker's settings needs this name.
pub fn model_file(kind: &str) -> String {
    format!("ggml-{kind}.en.bin")
}
