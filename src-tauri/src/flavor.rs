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
        _ => vec!["tiny", "base", "small", "medium"],
    }
}

pub fn default_model() -> &'static str {
    models().first().copied().unwrap_or("base")
}
