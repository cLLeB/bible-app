//! Presentation themes — the look applied over any `ProjectionState` on the
//! congregation screen. A theme is pure data (background + text style), so it
//! serialises straight to the projection window and persists as JSON in the
//! `settings` table. This is the visual foundation every later presentation
//! feature (media, alerts, songs, verses) renders through.
//!
//! Built-in themes ship in code and can't be deleted (only duplicated + edited
//! into a custom theme). Custom themes live in `settings` under the
//! `theme:<id>` prefix. `all_themes` = built-ins first, then custom.

use serde::{Deserialize, Serialize};

use crate::db::Db;

/// Where the congregation screen's background comes from. `image`/video land in
/// the next (media) slice; the variant space is kept forward-compatible so that
/// is a render addition, not a data-model change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Background {
    /// "color" (solid) | "gradient" (linear color→color2) | "image" | "video".
    pub kind: String,
    /// Solid fill, or the gradient's first stop. Also shows under a still-loading
    /// image/video, so text is legible before media paints.
    pub color: String,
    /// Gradient's second stop (ignored when kind == "color").
    pub color2: String,
    /// Gradient angle in degrees (0 = upward).
    pub angle: u16,
    /// Absolute path to an image/video file (empty for color/gradient).
    #[serde(default)]
    pub src: String,
    /// How media fills the screen: "cover" | "contain".
    #[serde(default = "default_fit")]
    pub fit: String,
    /// 0..1 dark overlay drawn over media so text stays readable.
    #[serde(default)]
    pub dim: f32,
}

fn default_fit() -> String {
    "cover".into()
}

/// How the words look: font, colour, weight, and the legibility aids that make
/// text readable over any background.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextStyle {
    /// A CSS font stack.
    pub font_family: String,
    /// Body colour.
    pub color: String,
    /// Reference/title caption colour.
    pub caption_color: String,
    /// "center" | "left" | "right".
    pub align: String,
    /// 400 (normal) | 600 (semibold) | 700 (bold).
    pub weight: u16,
    /// Drop a soft shadow behind the text for legibility over busy backgrounds.
    pub shadow: bool,
    /// Render body text in uppercase.
    pub uppercase: bool,
}

/// Where things sit on the slide, as opposed to how they look.
///
/// `TextStyle` already carries the look - font, colour, weight. This is the
/// arrangement, and it is part of a theme rather than a global setting so a church
/// can keep more than one and switch between them: a reference tucked under the
/// verse for a teaching series, hidden entirely for a reading, the words held high
/// on the screen when a lower third is being keyed over the stream.
///
/// Every field has a default matching what the app did before this existed, so a
/// theme saved by an older build loads unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// "below" | "above" | "hidden" - where the reference or song title goes.
    #[serde(default = "default_caption_position")]
    pub caption_position: String,
    /// "center" | "top" | "bottom" - where the body sits in the frame.
    #[serde(default = "default_vertical")]
    pub vertical: String,
    /// Caption size relative to the body. 1.0 is the old fixed relationship.
    #[serde(default = "default_caption_scale")]
    pub caption_scale: f32,
    /// Leave room down the sides. A percentage of the screen width, per side, so
    /// text does not run into the bezel or under a stream's lower third.
    #[serde(default = "default_side_margin")]
    pub side_margin: f32,
}

fn default_caption_position() -> String {
    "below".into()
}
fn default_vertical() -> String {
    "center".into()
}
fn default_caption_scale() -> f32 {
    1.0
}
fn default_side_margin() -> f32 {
    4.0
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            caption_position: default_caption_position(),
            vertical: default_vertical(),
            caption_scale: default_caption_scale(),
            side_margin: default_side_margin(),
        }
    }
}

/// A named, self-contained look for the congregation screen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub background: Background,
    pub text: TextStyle,
    /// Where things sit. Defaulted, so themes saved before it existed still load.
    #[serde(default)]
    pub layout: Layout,
    /// Ships in code; can be duplicated + edited but never deleted.
    pub built_in: bool,
}

const FONT_SANS: &str = "Inter, 'Segoe UI', system-ui, sans-serif";
const FONT_SERIF: &str = "Georgia, 'Times New Roman', serif";

fn solid(color: &str) -> Background {
    Background {
        kind: "color".into(),
        color: color.into(),
        color2: color.into(),
        angle: 0,
        src: String::new(),
        fit: default_fit(),
        dim: 0.0,
    }
}

fn gradient(color: &str, color2: &str, angle: u16) -> Background {
    Background {
        kind: "gradient".into(),
        color: color.into(),
        color2: color2.into(),
        angle,
        src: String::new(),
        fit: default_fit(),
        dim: 0.0,
    }
}

fn text(font_family: &str, color: &str, caption_color: &str, weight: u16, shadow: bool) -> TextStyle {
    TextStyle {
        font_family: font_family.into(),
        color: color.into(),
        caption_color: caption_color.into(),
        align: "center".into(),
        weight,
        shadow,
        uppercase: false,
    }
}

/// The look the app shipped with, plus a few richer ones. Order is display order.
/// The first three (`dark`/`light`/`sepia`) preserve the pre-theme appearance so
/// nothing regresses for existing users.
pub fn builtin_themes() -> Vec<Theme> {
    vec![
        Theme {
            id: "dark".into(),
            name: "Dark".into(),
            background: solid("#000000"),
            text: text(FONT_SANS, "#ffffff", "#c9c9c9", 400, false),
            layout: Layout::default(),
            built_in: true,
        },
        Theme {
            id: "light".into(),
            name: "Light".into(),
            background: solid("#ffffff"),
            text: text(FONT_SANS, "#101010", "#555555", 400, false),
            layout: Layout::default(),
            built_in: true,
        },
        Theme {
            id: "sepia".into(),
            name: "Sepia".into(),
            background: solid("#f4ecd8"),
            text: text(FONT_SERIF, "#5b4636", "#8a725a", 400, false),
            layout: Layout::default(),
            built_in: true,
        },
        Theme {
            id: "spotlight".into(),
            name: "Spotlight".into(),
            background: gradient("#000000", "#161616", 160),
            text: text(FONT_SANS, "#ffffff", "#d0d0d0", 700, true),
            layout: Layout::default(),
            built_in: true,
        },
        Theme {
            id: "ocean".into(),
            name: "Ocean".into(),
            background: gradient("#0b1e3a", "#0a1024", 160),
            text: text(FONT_SANS, "#f5f8ff", "#9db4d6", 600, true),
            layout: Layout::default(),
            built_in: true,
        },
    ]
}

/// The id used when nothing has been chosen yet — the shipped default look.
pub fn default_theme_id() -> &'static str {
    "dark"
}

/// The default active theme (dark), used before any settings load.
pub fn default_theme() -> Theme {
    builtin_themes()
        .into_iter()
        .find(|t| t.id == default_theme_id())
        .expect("dark is a built-in")
}

const CUSTOM_PREFIX: &str = "theme:";

/// Custom themes saved by the operator, oldest-key first.
pub fn custom_themes(db: &Db) -> Vec<Theme> {
    db.settings_with_prefix(CUSTOM_PREFIX)
        .into_iter()
        .filter_map(|(_, json)| serde_json::from_str::<Theme>(&json).ok())
        .map(|t| Theme { built_in: false, ..t })
        .collect()
}

/// Every theme the operator can pick: built-ins first, then their custom ones.
pub fn all_themes(db: &Db) -> Vec<Theme> {
    let mut out = builtin_themes();
    out.extend(custom_themes(db));
    out
}

/// Resolve an id to a theme, falling back to the default so the screen always
/// has a valid look even if a since-deleted theme is still selected.
pub fn theme_by_id(db: &Db, id: &str) -> Theme {
    all_themes(db).into_iter().find(|t| t.id == id).unwrap_or_else(default_theme)
}

/// Upsert a custom theme (never a built-in — built-ins are duplicated into a new
/// id before editing). Returns an error only on a storage failure.
pub fn save_custom(db: &Db, theme: &Theme) -> rusqlite::Result<()> {
    let stored = Theme { built_in: false, ..theme.clone() };
    let json = serde_json::to_string(&stored).unwrap_or_default();
    db.set_setting(&format!("{CUSTOM_PREFIX}{}", theme.id), &json)
}

/// Remove a custom theme. Built-in ids are ignored (they aren't stored).
pub fn delete_custom(db: &Db, id: &str) -> rusqlite::Result<()> {
    db.delete_setting(&format!("{CUSTOM_PREFIX}{id}"))
}

const ACTIVE_KEY: &str = "projection:active_theme";
const SCALE_KEY: &str = "projection:font_scale";

/// The persisted active theme id, or the shipped default when unset.
pub fn active_theme_id(db: &Db) -> String {
    db.get_setting(ACTIVE_KEY).unwrap_or_else(|| default_theme_id().to_string())
}

pub fn set_active_theme_id(db: &Db, id: &str) -> rusqlite::Result<()> {
    db.set_setting(ACTIVE_KEY, id)
}

/// The persisted global font multiplier, or 1.0 when unset/unparseable.
pub fn font_scale(db: &Db) -> f32 {
    db.get_setting(SCALE_KEY).and_then(|s| s.parse().ok()).unwrap_or(1.0)
}

pub fn set_font_scale(db: &Db, scale: f32) -> rusqlite::Result<()> {
    db.set_setting(SCALE_KEY, &scale.to_string())
}

/// Rebuild the resolved projection settings (scale + active theme) from storage,
/// so appearance survives a restart. Falls back to defaults on anything missing.
pub fn load_projection_settings(db: &Db) -> crate::events::ProjectionSettings {
    crate::events::ProjectionSettings {
        font_scale: font_scale(db),
        theme: theme_by_id(db, &active_theme_id(db)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_stable_and_flagged() {
        let ids: Vec<_> = builtin_themes().iter().map(|t| t.id.clone()).collect();
        assert_eq!(ids, ["dark", "light", "sepia", "spotlight", "ocean"]);
        assert!(builtin_themes().iter().all(|t| t.built_in));
        // The shipped look must not silently change.
        let dark = default_theme();
        assert_eq!(dark.background.color, "#000000");
        assert_eq!(dark.text.color, "#ffffff");
        assert_eq!(dark.text.caption_color, "#c9c9c9");
    }

    #[test]
    fn theme_round_trips_camel_case() {
        let t = default_theme();
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"fontFamily\""), "got {json}");
        assert!(json.contains("\"captionColor\""), "got {json}");
        assert!(json.contains("\"builtIn\""), "got {json}");
        let back: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }

    /// A theme saved before layouts existed must load and look exactly as it did.
    ///
    /// Themes are stored as JSON, so every field added later is a chance to break
    /// what a church already has. This is the same guard the media fields have.
    #[test]
    fn old_themes_without_a_layout_still_load() {
        let json = r##"{
            "id": "custom",
            "name": "Ours",
            "background": {"kind":"color","color":"#101010","color2":"#000000","angle":0,"src":"","fit":"cover","dim":0},
            "text": {"fontFamily":"serif","color":"#fff","captionColor":"#ccc","align":"center","weight":400,"shadow":false,"uppercase":false},
            "builtIn": false
        }"##;
        let t: Theme = serde_json::from_str(json).expect("a theme without a layout still loads");
        assert_eq!(t.layout, Layout::default());
        assert_eq!(t.layout.caption_position, "below", "the reference stays where it was");
        assert_eq!(t.layout.vertical, "center");
    }

    #[test]
    fn old_themes_without_media_fields_still_load() {
        // A theme persisted before media backgrounds existed must deserialize
        // with sensible defaults (no src, cover fit, no dim) — not fail.
        let legacy = r##"{"id":"x","name":"X","background":{"kind":"color","color":"#111111","color2":"#111111","angle":0},"text":{"fontFamily":"Inter","color":"#fff","captionColor":"#ccc","align":"center","weight":400,"shadow":false,"uppercase":false},"builtIn":false}"##;
        let t: Theme = serde_json::from_str(legacy).unwrap();
        assert_eq!(t.background.src, "");
        assert_eq!(t.background.fit, "cover");
        assert_eq!(t.background.dim, 0.0);
    }

    #[test]
    fn custom_themes_survive_save_list_delete() {
        let db = crate::db::open_in_memory().unwrap();
        db.migrate().unwrap();
        let mut mine = default_theme();
        mine.id = "mine".into();
        mine.name = "Mine".into();
        mine.background = solid("#123456");
        // Saving forces built_in=false even if a built-in was duplicated.
        mine.built_in = true;
        save_custom(&db, &mine).unwrap();

        let custom = custom_themes(&db);
        assert_eq!(custom.len(), 1);
        assert_eq!(custom[0].id, "mine");
        assert!(!custom[0].built_in, "stored custom themes are never built-in");
        assert_eq!(custom[0].background.color, "#123456");

        // all_themes appends custom after the built-ins.
        let all = all_themes(&db);
        assert_eq!(all.len(), builtin_themes().len() + 1);
        assert_eq!(all.last().unwrap().id, "mine");
        assert_eq!(theme_by_id(&db, "mine").name, "Mine");

        delete_custom(&db, "mine").unwrap();
        assert!(custom_themes(&db).is_empty());
        // A now-missing selection falls back to the default, never panics.
        assert_eq!(theme_by_id(&db, "mine").id, default_theme_id());
    }
}
