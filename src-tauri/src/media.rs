//! The media library and the slideshow that walks it.
//!
//! Two decisions shape this module.
//!
//! **The library holds references, not copies.** A church's media folder is
//! measured in gigabytes; importing would mean a second copy, a slow import, and
//! a library that silently rots when someone tidies the original folder. Adding
//! a file records its path. A file that has since moved is reported as missing
//! rather than pretended about.
//!
//! **The slideshow runs in the backend.** It has to keep advancing while the
//! operator switches console tabs, opens the theme editor, or walks away from
//! the laptop entirely, and it has to be controllable from the phone. A timer
//! living in a React component satisfies none of that: the console learned this
//! the hard way when the stage monitor stopped following the phone because the
//! component holding the listener was on the other tab.

use crate::commands::AppState;
use crate::events::{ProjectionState, StageSlot};
use base64::Engine;
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Extensions the projection window can actually show. Kept in step with
/// `src/lib/media.ts`, which offers them in the file picker.
const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp", "avif"];
const VIDEO_EXT: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv", "avi"];
/// Sound-only files. These live in the same library as the pictures because an
/// operator thinks of "the things I brought for Sunday" as one pile, but they
/// never take the screen — see `events::AudioState`.
const AUDIO_EXT: &[&str] = &["mp3", "m4a", "wav", "ogg", "oga", "flac", "aac", "opus"];

/// How long a slideshow holds each item, and the bounds a typed value is held
/// to. The floor exists because a mistyped 0 would otherwise flash the whole
/// library past the congregation in a second.
pub const MIN_SECONDS: u64 = 2;
pub const MAX_SECONDS: u64 = 600;
pub const DEFAULT_SECONDS: u64 = 8;

/// How often the runner wakes to notice a stop. Short enough that Stop feels
/// immediate, long enough that an idle slideshow costs nothing.
const TICK: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub kind: String,
    /// The document this page came from, empty for a standalone file. Pages of
    /// one deck stay together, and stepping moves inside it.
    pub deck: String,
    /// False when the file is no longer where it was added from. The operator
    /// finds out in the library, not when it fails to appear on the wall.
    pub present: bool,
}

/// What kind of media a path is, or None when it is not something we can show.
pub fn kind_of(path: &str) -> Option<&'static str> {
    let ext = Path::new(path).extension()?.to_str()?.to_lowercase();
    if IMAGE_EXT.contains(&ext.as_str()) {
        return Some("image");
    }
    if VIDEO_EXT.contains(&ext.as_str()) {
        return Some("video");
    }
    if AUDIO_EXT.contains(&ext.as_str()) {
        return Some("audio");
    }
    None
}

/// The file's own name, without directories or extension, as a starting title.
pub fn title_of(path: &str) -> String {
    let stem = Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if stem.trim().is_empty() {
        Path::new(path).file_name().and_then(|s| s.to_str()).unwrap_or(path).to_string()
    } else {
        stem.to_string()
    }
}

/// A dwell time from whatever arrived, held to the bounds above.
pub fn clamp_seconds(seconds: u64) -> u64 {
    if seconds == 0 {
        return DEFAULT_SECONDS;
    }
    seconds.clamp(MIN_SECONDS, MAX_SECONDS)
}

/// The next index, or None when the run is over. Looping never ends, so an
/// announcements loop can be left running before a service starts.
pub fn next_index(current: usize, count: usize, looping: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let next = current + 1;
    if next < count {
        Some(next)
    } else if looping {
        Some(0)
    } else {
        None
    }
}

/// The projection state for one library item. Videos start playing, unmuted and
/// not looping: a bumper that silently loops forever is the wrong default, and
/// the operator can set either from the controls.
pub fn state_for(path: &str, title: &str, kind: &str) -> ProjectionState {
    if kind == "video" {
        ProjectionState::Video {
            src: path.to_string(),
            title: title.to_string(),
            paused: false,
            muted: false,
            looping: false,
        }
    } else {
        ProjectionState::Image { src: path.to_string() }
    }
}

/// A file-system-safe folder name for a deck, so two imports of "Sunday.pdf"
/// and "sunday .pdf" cannot collide or escape the slides folder.
pub fn slug(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let trimmed = cleaned.trim_matches('-').to_string();
    let short: String = trimmed.chars().take(60).collect();
    if short.is_empty() {
        "deck".into()
    } else {
        short
    }
}

/// Where rendered slide pages live: inside the app's own data directory, so a
/// deck imported on Saturday is still there on Sunday and survives a restart.
/// The previous approach kept pages only as data URLs in a React component,
/// which meant re-importing before every service and pushing megabytes of text
/// through the event channel on every single page change.
pub fn slides_dir(app: &AppHandle, deck: &str) -> Result<std::path::PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("slides")
        .join(slug(deck));
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    Ok(base)
}

/// Presentation formats that must be converted before we can render them.
/// PDF is not here: it is the format we render directly.
const DECK_EXT: &[&str] = &["pptx", "ppt", "odp", "pps", "ppsx"];

/// Does this file need converting before it can be turned into slides?
pub fn needs_conversion(path: &str) -> bool {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some(ext) => DECK_EXT.contains(&ext.to_lowercase().as_str()),
        None => false,
    }
}

/// How a given converter binary wants to be driven.
///
/// Every office suite solves this differently, and the differences are not
/// cosmetic: one takes an output *directory* and names the file itself, another
/// takes the output path, and a third has no command line at all and must be
/// automated as an application.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConvertStyle {
    /// `soffice --headless --convert-to pdf --outdir <dir> <file>`, which names
    /// the result `<stem>.pdf` itself. LibreOffice, OpenOffice, and the several
    /// suites that ship a compatible `soffice`.
    SofficeHeadless,
    /// `x2t <from> <to>`. ONLYOFFICE's own converter core.
    X2t,
    /// No command line: the application is driven through COM. The string is
    /// the ProgID. Microsoft PowerPoint and WPS Presentation both answer to
    /// `Presentations.Open` / `SaveAs(..., 32)`.
    WindowsCom(&'static str),
}

/// A converter this machine could use.
#[derive(Debug, Clone)]
pub struct Converter {
    pub name: String,
    pub program: std::path::PathBuf,
    pub style: ConvertStyle,
}

/// Where each known suite keeps its converter, relative to a Windows program
/// directory. Order is preference order: headless command-line converters come
/// before COM automation, which launches a real application and can stall on a
/// dialog nobody is there to dismiss on a Sunday morning.
const WINDOWS_CANDIDATES: &[(&str, &str, ConvertStyle)] = &[
    ("LibreOffice", "LibreOffice/program/soffice.exe", ConvertStyle::SofficeHeadless),
    ("ONLYOFFICE", "ONLYOFFICE/DesktopEditors/converter/x2t.exe", ConvertStyle::X2t),
    ("OpenOffice", "OpenOffice 4/program/soffice.exe", ConvertStyle::SofficeHeadless),
    ("OpenOffice", "OpenOffice/program/soffice.exe", ConvertStyle::SofficeHeadless),
    ("WPS Office", "Kingsoft/WPS Office/office6/wpp.exe", ConvertStyle::WindowsCom("Kwpp.Application")),
];

/// Non-Windows locations, so this is not silently a Windows-only feature.
const UNIX_CANDIDATES: &[(&str, &str, ConvertStyle)] = &[
    ("LibreOffice", "/usr/bin/soffice", ConvertStyle::SofficeHeadless),
    ("LibreOffice", "/usr/local/bin/soffice", ConvertStyle::SofficeHeadless),
    (
        "LibreOffice",
        "/Applications/LibreOffice.app/Contents/MacOS/soffice",
        ConvertStyle::SofficeHeadless,
    ),
    ("ONLYOFFICE", "/opt/onlyoffice/desktopeditors/converter/x2t", ConvertStyle::X2t),
    (
        "ONLYOFFICE",
        "/Applications/ONLYOFFICE.app/Contents/MacOS/converter/x2t",
        ConvertStyle::X2t,
    ),
];

/// Infer how to drive a binary the operator pointed at by hand. Only the file
/// name can be trusted here: the operator may have installed anywhere.
pub fn style_for(program: &Path) -> Option<ConvertStyle> {
    let stem = program.file_stem()?.to_str()?.to_lowercase();
    match stem.as_str() {
        "soffice" | "soffice.bin" | "libreoffice" => Some(ConvertStyle::SofficeHeadless),
        "x2t" => Some(ConvertStyle::X2t),
        _ => None,
    }
}

/// Every converter this machine can offer, best first.
///
/// A church buys whatever office suite it buys, and a feature that works only
/// for LibreOffice users is a feature most churches do not have. So the known
/// suites are all looked for, and an operator whose install is somewhere else
/// entirely can name the binary themselves.
pub fn converters(app: &AppHandle) -> Vec<Converter> {
    let mut found: Vec<Converter> = Vec::new();

    // The operator's own choice outranks anything discovered.
    if let Some(chosen) = converter_override(app) {
        let path = std::path::PathBuf::from(&chosen);
        if path.exists() {
            if let Some(style) = style_for(&path) {
                found.push(Converter { name: "Chosen converter".into(), program: path, style });
            }
        }
    }

    let mut bases: Vec<String> = Vec::new();
    for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Ok(base) = std::env::var(var) {
            if !bases.contains(&base) {
                bases.push(base);
            }
        }
    }
    for (name, relative, style) in WINDOWS_CANDIDATES {
        for base in &bases {
            let path = Path::new(base).join(relative);
            if path.exists() && !found.iter().any(|c| c.program == path) {
                found.push(Converter { name: (*name).into(), program: path, style: *style });
            }
        }
    }
    for (name, absolute, style) in UNIX_CANDIDATES {
        let path = std::path::PathBuf::from(absolute);
        if path.exists() && !found.iter().any(|c| c.program == path) {
            found.push(Converter { name: (*name).into(), program: path, style: *style });
        }
    }

    // A suite on PATH, which is how most Linux installs and some Windows ones
    // look. Existence cannot be tested for a bare name, so it goes last and is
    // simply attempted.
    found.push(Converter {
        name: "LibreOffice (PATH)".into(),
        program: "soffice".into(),
        style: ConvertStyle::SofficeHeadless,
    });

    #[cfg(windows)]
    found.push(Converter {
        name: "Microsoft PowerPoint".into(),
        program: "powershell".into(),
        style: ConvertStyle::WindowsCom("PowerPoint.Application"),
    });

    found
}

const CONVERTER_KEY: &str = "media:converter";

fn converter_override(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    let db = state.db.lock().ok()?;
    db.get_setting(CONVERTER_KEY)
}

/// Remember a converter the operator picked themselves. An empty path clears it.
pub fn set_converter_override(app: &AppHandle, path: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_setting(CONVERTER_KEY, path.trim()).map_err(|e| e.to_string())
}

/// Convert a PowerPoint or OpenDocument deck to PDF, which we already render
/// well, and return the PDF's path.
///
/// Rendering PowerPoint faithfully means fonts, layouts, shapes and charts:
/// an engine, not a parser. Rather than bundle hundreds of megabytes into an
/// installer that is already large, this borrows an engine the machine already
/// has. Most churches have one of the two. When neither is present the operator
/// is told exactly what to do instead, because a silent failure here would look
/// like the app simply refusing their file.
pub fn convert_to_pdf(app: &AppHandle, path: &str) -> Result<String, String> {
    if !Path::new(path).exists() {
        return Err(format!("'{path}' is not there any more."));
    }
    let out_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("converted");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

    let stem = Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("deck");
    let expected = out_dir.join(format!("{stem}.pdf"));
    // A previous conversion of the same deck is reused only if it is newer than
    // the source, so an edited deck is genuinely re-converted.
    if is_fresh(&expected, path) {
        let found = expected.to_string_lossy().to_string();
        allow_path(app, &found);
        return Ok(found);
    }

    let mut tried: Vec<String> = Vec::new();
    for converter in converters(app) {
        match run_converter(&converter, path, &out_dir, &expected) {
            Ok(()) if expected.exists() => {
                let found = expected.to_string_lossy().to_string();
                allow_path(app, &found);
                return Ok(found);
            }
            Ok(()) => tried.push(format!("{} ran but produced nothing", converter.name)),
            Err(e) => tried.push(format!("{}: {e}", converter.name)),
        }
    }

    let detail = if tried.is_empty() { String::new() } else { format!(" Tried: {}.", tried.join("; ")) };
    Err(format!(
        "No office suite on this machine could convert that deck.{detail} Install LibreOffice \
         or ONLYOFFICE (both free), point the app at a converter you already have, or export \
         the deck to PDF and import that."
    ))
}

/// Drive one converter in whichever way it expects.
fn run_converter(
    converter: &Converter,
    input: &str,
    out_dir: &Path,
    out_file: &Path,
) -> Result<(), String> {
    match converter.style {
        ConvertStyle::SofficeHeadless => {
            let mut cmd = std::process::Command::new(&converter.program);
            cmd.args(["--headless", "--norestore", "--convert-to", "pdf", "--outdir"])
                .arg(out_dir)
                .arg(input);
            run(cmd, &converter.program)
        }
        ConvertStyle::X2t => {
            // Measured, not assumed: the bare `x2t <in> <out>` form fails with
            // exit 89 and no output file. What works is the documented task
            // file, and it must name the font cache ONLYOFFICE's own editor
            // generates — without real fonts there is nothing to draw a PDF
            // with. Same deck, same binary: 89 and nothing, versus a PDF.
            let params = write_x2t_params(&converter.program, input, out_file, out_dir)?;
            let mut cmd = std::process::Command::new(&converter.program);
            cmd.arg(&params);
            // x2t loads its own libraries from beside itself, so it has to be
            // run from its own directory rather than from ours.
            if let Some(dir) = converter.program.parent() {
                cmd.current_dir(dir);
            }
            run(cmd, &converter.program)
        }
        ConvertStyle::WindowsCom(prog_id) => convert_with_com(prog_id, input, out_file),
    }
}

/// ONLYOFFICE's PDF format id, from its own format table.
const X2T_FORMAT_PDF: u32 = 513;

/// Write the task file x2t actually wants, and return its path.
///
/// The paths are all derived from the converter binary the registry found, so
/// this works for an install anywhere, not only the default one. The font cache
/// is looked for where the desktop editor keeps it; when it is missing, x2t is
/// pointed at a path in our own data directory and builds one there.
fn write_x2t_params(
    program: &Path,
    input: &str,
    out_file: &Path,
    work_dir: &Path,
) -> Result<std::path::PathBuf, String> {
    // <install>/converter/x2t.exe -> <install>
    let root = program
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| "converter is in an unexpected place".to_string())?;

    let cached = std::env::var("LOCALAPPDATA")
        .ok()
        .map(|local| Path::new(&local).join("ONLYOFFICE/DesktopEditors/data/fonts/AllFonts.js"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| work_dir.join("AllFonts.js"));

    let params = work_dir.join("x2t-task.xml");
    let xml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<TaskQueueDataConvert xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
  <m_sFileFrom>{from}</m_sFileFrom>
  <m_sFileTo>{to}</m_sFileTo>
  <m_nFormatTo>{format}</m_nFormatTo>
  <m_sFontDir>{fonts}</m_sFontDir>
  <m_sAllFontsPath>{cache}</m_sAllFontsPath>
  <m_sThemeDir>{themes}</m_sThemeDir>
</TaskQueueDataConvert>
"#,
        from = xml_escape(input),
        to = xml_escape(&out_file.to_string_lossy()),
        format = X2T_FORMAT_PDF,
        fonts = xml_escape(&root.join("fonts").to_string_lossy()),
        cache = xml_escape(&cached.to_string_lossy()),
        themes = xml_escape(&root.join("editors/sdkjs/slide/themes").to_string_lossy()),
    );
    std::fs::write(&params, xml).map_err(|e| e.to_string())?;
    Ok(params)
}

/// A church deck can be called anything, including "Q&A <live>".
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn run(mut cmd: std::process::Command, program: &Path) -> Result<(), String> {
    match cmd.output() {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let why = String::from_utf8_lossy(&out.stderr);
            let why = why.trim();
            Err(if why.is_empty() { format!("exited with {}", out.status) } else { why.into() })
        }
        Err(e) => Err(format!("could not start {} ({e})", program.display())),
    }
}

/// An office application driven through COM. `32` is the PDF save format, which
/// Microsoft PowerPoint and WPS Presentation both use.
#[cfg(windows)]
fn convert_with_com(prog_id: &str, input: &str, out: &Path) -> Result<(), String> {
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         $app = New-Object -ComObject {prog_id}; \
         $deck = $app.Presentations.Open('{}', $true, $false, $false); \
         $deck.SaveAs('{}', 32); $deck.Close(); $app.Quit()",
        input.replace('\'', "''"),
        out.to_string_lossy().replace('\'', "''"),
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| format!("could not start PowerShell ({e})"))?;
    if out.exists() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[cfg(not(windows))]
fn convert_with_com(_prog_id: &str, _input: &str, _out: &Path) -> Result<(), String> {
    Err("COM automation is Windows only".into())
}

/// True when `built` exists and is at least as new as `source`.
fn is_fresh(built: &Path, source: &str) -> bool {
    let (Ok(a), Ok(b)) = (std::fs::metadata(built), std::fs::metadata(source)) else {
        return false;
    };
    match (a.modified(), b.modified()) {
        (Ok(built_at), Ok(source_at)) => built_at >= source_at,
        _ => false,
    }
}

/// Let the projection window actually load this file.
///
/// The asset protocol is scoped, and the configured scope covers the user's own
/// folders. Church media does not live there: it lives on a second drive or a
/// mapped NAS share, which the picker will happily add and the webview would
/// then refuse to load, showing black. Rather than open the scope to the whole
/// disk, each file the operator deliberately adds is allowed by name.
///
/// Failures are ignored on purpose: a path that cannot be allowed will surface
/// as that item failing to display, which is information the operator can act
/// on, where a startup error about a file they have forgotten about is not.
pub fn allow_path(app: &AppHandle, path: &str) {
    let _ = app.asset_protocol_scope().allow_file(path);
}

/// Re-allow everything the library already holds. The scope is per-run, so
/// without this a file added last Sunday is refused after the next restart.
pub fn allow_known_paths(app: &AppHandle) {
    for item in list(app) {
        allow_path(app, &item.path);
    }
    // Theme backgrounds are chosen from the same kind of folder and were
    // scoped the same way.
    let state = app.state::<AppState>();
    // Bound to a local so the guard drops before `state` does.
    let src = match state.settings.lock() {
        Ok(ref s) => s.theme.background.src.clone(),
        Err(_) => String::new(),
    };
    if !src.trim().is_empty() {
        allow_path(app, &src);
    }
}

/// Write one rendered page to disk and put it in the library, so a deck's pages
/// are ordinary media: previewable, projectable, orderable, and usable as
/// service cues or slideshow items like anything else.
pub fn save_slide(
    app: &AppHandle,
    deck: &str,
    index: u32,
    encoded: &str,
) -> Result<MediaItem, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|_| "That page could not be decoded.".to_string())?;
    let dir = slides_dir(app, deck)?;
    // Zero-padded so a 100-page deck still sorts as a human reads it.
    let path = dir.join(format!("page-{index:03}.png"));
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    let path_str = path.to_string_lossy().to_string();
    let title = format!("{} · {}", deck.trim(), index);
    {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.add_media(&path_str, &title, "image", deck.trim()).map_err(|e| e.to_string())?;
    }
    allow_path(app, &path_str);
    list(app)
        .into_iter()
        .find(|m| m.path == path_str)
        .ok_or_else(|| "The page was written but not listed.".to_string())
}

pub fn list(app: &AppHandle) -> Vec<MediaItem> {
    let state = app.state::<AppState>();
    let rows = match state.db.lock() {
        Ok(db) => db.list_media().unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    rows.into_iter()
        .map(|(id, path, title, kind, deck)| MediaItem {
            present: Path::new(&path).exists(),
            id,
            path,
            title,
            kind,
            deck,
        })
        .collect()
}

/// Move to the previous or next page of the same deck, and show it.
///
/// This is what stepping through a deck means, and it is a different thing from
/// the announcements loop: next and previous stay inside the document being
/// presented rather than wandering into whatever else is in the library.
/// Returns None at either end of the deck, or for a standalone file.
pub fn step_deck(app: &AppHandle, id: i64, forward: bool) -> Option<MediaItem> {
    let items = list(app);
    let current = items.iter().find(|m| m.id == id)?;
    if current.deck.is_empty() {
        return None;
    }
    let pages: Vec<&MediaItem> = items.iter().filter(|m| m.deck == current.deck).collect();
    let at = pages.iter().position(|m| m.id == id)?;
    let target = if forward { at.checked_add(1)? } else { at.checked_sub(1)? };
    let wanted = pages.get(target)?.id;
    present(app, wanted).ok()
}

/// Put one library item on the screen, and name it on the stage monitor with
/// whatever follows it, so the platform team sees what is coming.
pub fn present(app: &AppHandle, id: i64) -> Result<MediaItem, String> {
    let items = list(app);
    let pos = items
        .iter()
        .position(|m| m.id == id)
        .ok_or_else(|| "That item is no longer in the library.".to_string())?;
    present_at(app, &items, pos)
}

fn present_at(app: &AppHandle, items: &[MediaItem], pos: usize) -> Result<MediaItem, String> {
    let item = items.get(pos).ok_or_else(|| "Nothing to show.".to_string())?;
    if !item.present {
        return Err(format!("'{}' is no longer at {}", item.title, item.path));
    }
    // A sound file is "put on" by playing it, not by projecting it. Routing that
    // here rather than at each caller means the run order, the console and the
    // phone all do the right thing with one, and none of them has to ask what
    // kind of file it is first.
    if !is_visual(&item.kind) {
        crate::commands::play_audio_handle(app, &item.path, &item.title)?;
        return Ok(item.clone());
    }
    crate::commands::project_via_handle(app, state_for(&item.path, &item.title, &item.kind))?;
    let next = items.get(pos + 1).map(|n| StageSlot {
        text: n.title.clone(),
        caption: kind_label(&n.kind).to_string(),
    });
    crate::commands::set_stage_handle(
        app,
        Some(StageSlot { text: item.title.clone(), caption: kind_label(&item.kind).to_string() }),
        next,
    );
    Ok(item.clone())
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "video" => "Video",
        "audio" => "Audio",
        _ => "Image",
    }
}

/// Does this item take the congregation screen? Audio does not, so anything
/// that walks the library *looking for something to show* has to ask.
pub fn is_visual(kind: &str) -> bool {
    kind != "audio"
}

/// The library, minus anything that never takes the screen. The announcements
/// loop walks this rather than the whole library: a sound file in the middle of
/// a loop would otherwise hold the last picture up for its dwell time and look
/// exactly like the loop had stuck.
fn visuals(app: &AppHandle) -> Vec<MediaItem> {
    list(app).into_iter().filter(|m| is_visual(&m.kind)).collect()
}

/// Longest a loop will wait on a video before giving up on hearing that it
/// ended. A file the projection window cannot decode would otherwise hold the
/// loop forever, which on a Sunday morning looks exactly like a crash.
const VIDEO_PATIENCE: Duration = Duration::from_secs(60 * 20);

/// Start the announcements loop. Idempotent: starting a running loop is a no-op
/// rather than a second thread racing the first for the screen.
///
/// A timer is the wrong unit for a video, so the loop does not use one there: a
/// video is held until it reports that it ended, then the loop moves on. The
/// dwell time governs images, which have no natural length of their own.
pub fn start_slideshow(
    app: AppHandle,
    running: Arc<AtomicBool>,
    seconds: u64,
    looping: bool,
) -> Result<(), String> {
    let items = visuals(&app);
    if items.iter().all(|m| !m.present) {
        return Err("The media library has nothing to show.".into());
    }
    if running.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let dwell = clamp_seconds(seconds);
    let _ = app.emit("slideshow-changed", true);

    std::thread::spawn(move || {
        let mut pos = 0usize;
        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }
            // Re-read every step: the operator may add, rename or remove
            // something while it runs, and the next slide should be the library
            // as it is now, not as it was when Start was pressed.
            let items = visuals(&app);
            if items.is_empty() {
                break;
            }
            if pos >= items.len() {
                pos = 0;
            }
            // Skip anything that has gone missing rather than stopping dead.
            let shown = present_at(&app, &items, pos);
            if shown.is_err() {
                match next_index(pos, items.len(), looping) {
                    Some(n) if n != 0 || looping => {
                        pos = n;
                        continue;
                    }
                    _ => break,
                }
            }
            let is_video = shown.map(|m| m.kind == "video").unwrap_or(false);
            let held = if is_video {
                wait_for_video(&app, &running)
            } else {
                sleep_unless_stopped(&running, dwell)
            };
            if !held {
                break;
            }
            match next_index(pos, items.len(), looping) {
                Some(n) => pos = n,
                None => break,
            }
        }
        running.store(false, Ordering::SeqCst);
        let _ = app.emit("slideshow-changed", false);
    });
    Ok(())
}

/// Hold until the projection window says the video ended, the operator stops,
/// or patience runs out. False when stopped.
fn wait_for_video(app: &AppHandle, running: &Arc<AtomicBool>) -> bool {
    let ended = app.state::<AppState>().video_ended.clone();
    ended.store(false, Ordering::SeqCst);
    let deadline = std::time::Instant::now() + VIDEO_PATIENCE;
    loop {
        if !running.load(Ordering::SeqCst) {
            return false;
        }
        if ended.load(Ordering::SeqCst) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return true;
        }
        std::thread::sleep(TICK);
    }
}

/// Sleep `seconds`, waking often enough to notice a stop. False when stopped.
fn sleep_unless_stopped(running: &Arc<AtomicBool>, seconds: u64) -> bool {
    let ticks = (seconds * 1000) / TICK.as_millis() as u64;
    for _ in 0..ticks {
        if !running.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(TICK);
    }
    running.load(Ordering::SeqCst)
}

pub fn stop_slideshow(app: &AppHandle, running: &Arc<AtomicBool>) {
    running.store(false, Ordering::SeqCst);
    let _ = app.emit("slideshow-changed", false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_what_the_screen_can_show() {
        assert_eq!(kind_of("C:/media/backdrop.JPG"), Some("image"));
        assert_eq!(kind_of("/srv/loop.webm"), Some("video"));
        assert_eq!(kind_of("notes.pdf"), None);
        assert_eq!(kind_of("README"), None);
    }

    #[test]
    fn sound_files_join_the_library_without_joining_the_screen() {
        // Audio is in the library because the operator brought it for Sunday
        // with everything else, but it must never be treated as something to
        // project: `is_visual` is what every screen-bound path asks.
        assert_eq!(kind_of("walk-in.MP3"), Some("audio"));
        assert_eq!(kind_of("/srv/offering.flac"), Some("audio"));
        assert_eq!(kind_of("testimony.m4a"), Some("audio"));
        assert!(!is_visual("audio"));
        assert!(is_visual("image"));
        assert!(is_visual("video"));
        assert_eq!(kind_label("audio"), "Audio");
    }

    #[test]
    fn the_console_and_the_backend_agree_on_what_media_is() {
        // A file the picker offers but the backend refuses is a split that only
        // shows up mid-service, so the two lists are checked against each other.
        let ts = include_str!("../../src/lib/media.ts");
        for ext in IMAGE_EXT.iter().chain(VIDEO_EXT.iter()).chain(AUDIO_EXT.iter()) {
            assert!(ts.contains(&format!("\"{ext}\"")), "{ext} is missing from src/lib/media.ts");
        }
    }

    #[test]
    fn each_suite_is_driven_the_way_it_expects() {
        // These are not interchangeable: soffice takes an output directory and
        // names the file itself, x2t takes the output path. Driving one like the
        // other produces no file and no useful error.
        assert_eq!(style_for(Path::new("C:/x/soffice.exe")), Some(ConvertStyle::SofficeHeadless));
        assert_eq!(style_for(Path::new("/usr/bin/soffice")), Some(ConvertStyle::SofficeHeadless));
        assert_eq!(
            style_for(Path::new("C:/Program Files/ONLYOFFICE/DesktopEditors/converter/x2t.exe")),
            Some(ConvertStyle::X2t)
        );
        // Something we have no idea how to drive must be refused, not guessed at.
        assert_eq!(style_for(Path::new("C:/x/notepad.exe")), None);
        assert_eq!(style_for(Path::new("x2t")), Some(ConvertStyle::X2t));
    }

    #[test]
    fn headless_converters_are_preferred_over_com_automation() {
        // COM launches a real application, which can stall on a dialog with
        // nobody there to dismiss it. It is a fallback, never a first choice.
        let com_first = WINDOWS_CANDIDATES
            .iter()
            .position(|(.., style)| matches!(style, ConvertStyle::WindowsCom(_)));
        let headless_last = WINDOWS_CANDIDATES
            .iter()
            .rposition(|(.., style)| !matches!(style, ConvertStyle::WindowsCom(_)));
        if let (Some(com), Some(headless)) = (com_first, headless_last) {
            assert!(headless < com, "a COM engine is being tried before a headless one");
        }
        // The suites a church is actually likely to have are all covered.
        let names: Vec<&str> = WINDOWS_CANDIDATES.iter().map(|(n, ..)| *n).collect();
        for expected in ["LibreOffice", "ONLYOFFICE", "OpenOffice", "WPS Office"] {
            assert!(names.contains(&expected), "{expected} is not looked for");
        }
    }

    #[test]
    fn a_deck_name_cannot_break_the_x2t_task_file() {
        // Church decks are called things like "Q&A <live>". Unescaped, that
        // produces malformed XML and a conversion that fails for a reason no
        // operator could ever guess.
        assert_eq!(xml_escape("Q&A <live>"), "Q&amp;A &lt;live&gt;");
        assert_eq!(xml_escape(r#"say "yes""#), "say &quot;yes&quot;");
        assert_eq!(xml_escape("C:/church/ordinary.pptx"), "C:/church/ordinary.pptx");
    }

    #[test]
    fn only_formats_needing_an_engine_are_converted() {
        // PDF is what we render directly, so sending it through a converter
        // would be a pointless round trip that can only lose fidelity.
        assert!(!needs_conversion("deck.pdf"));
        assert!(!needs_conversion("photo.png"));
        assert!(!needs_conversion("clip.mp4"));
        for deck in ["Sunday.pptx", "old.PPT", "notes.odp", "show.ppsx"] {
            assert!(needs_conversion(deck), "{deck} should be converted first");
        }
        assert!(!needs_conversion("README"));
    }

    #[test]
    fn deck_folders_cannot_collide_or_escape() {
        assert_eq!(slug("Sunday Morning"), "sunday-morning");
        // Two decks that differ only by punctuation must not share a folder in
        // a way that lets one overwrite the other's pages.
        assert_ne!(slug("Sunday Morning"), slug("SundayMorning"));
        // Nothing that could climb out of the slides directory survives.
        for hostile in ["../../etc/passwd", "..\\..\\windows", "C:/Windows/system32"] {
            let s = slug(hostile);
            assert!(!s.contains('/') && !s.contains('\\') && !s.contains(".."), "got {s}");
        }
        assert_eq!(slug(""), "deck");
        assert_eq!(slug("///"), "deck");
        assert!(slug(&"x".repeat(500)).len() <= 60);
    }

    #[test]
    fn titles_start_from_the_file_name() {
        assert_eq!(title_of("C:\\church\\Advent Week 1.mp4"), "Advent Week 1");
        assert_eq!(title_of("/srv/media/offering.png"), "offering");
    }

    #[test]
    fn a_mistyped_dwell_cannot_flash_the_library_past_the_room() {
        assert_eq!(clamp_seconds(0), DEFAULT_SECONDS);
        assert_eq!(clamp_seconds(1), MIN_SECONDS);
        assert_eq!(clamp_seconds(8), 8);
        assert_eq!(clamp_seconds(99_999), MAX_SECONDS);
    }

    #[test]
    fn walks_forward_and_wraps_only_when_looping() {
        assert_eq!(next_index(0, 3, false), Some(1));
        assert_eq!(next_index(2, 3, false), None);
        assert_eq!(next_index(2, 3, true), Some(0));
        assert_eq!(next_index(0, 0, true), None);
    }

    #[test]
    fn video_starts_playing_and_images_are_plain() {
        match state_for("/m/bumper.mp4", "Bumper", "video") {
            ProjectionState::Video { paused, looping, muted, title, .. } => {
                assert!(!paused, "a video the operator just chose should play");
                assert!(!looping, "looping forever is not a safe default for a bumper");
                assert!(!muted);
                assert_eq!(title, "Bumper");
            }
            other => panic!("expected a video, got {other:?}"),
        }
        assert_eq!(
            state_for("/m/slide.png", "Slide", "image"),
            ProjectionState::Image { src: "/m/slide.png".into() }
        );
    }

    #[test]
    fn library_rows_survive_add_reorder_and_remove() {
        let db = crate::db::open_in_memory().unwrap();
        db.migrate().unwrap();
        let a = db.add_media("/m/a.png", "A", "image", "").unwrap();
        let b = db.add_media("/m/b.mp4", "B", "video", "").unwrap();

        // Adding the same folder again must not multiply what is there.
        assert_eq!(db.add_media("/m/a.png", "A", "image", "").unwrap(), a);
        assert_eq!(db.list_media().unwrap().len(), 2);

        assert!(db.move_media(b, true).unwrap(), "B should move up");
        let order: Vec<i64> = db.list_media().unwrap().into_iter().map(|(id, ..)| id).collect();
        assert_eq!(order, vec![b, a]);

        // The ends of the list have nothing to swap with.
        assert!(!db.move_media(b, true).unwrap());
        assert!(!db.move_media(a, false).unwrap());

        db.rename_media(a, "Offering").unwrap();
        assert_eq!(db.media_at(a).unwrap().unwrap().1, "Offering");

        db.remove_media(a).unwrap();
        assert_eq!(db.list_media().unwrap().len(), 1);
        assert!(db.media_at(a).unwrap().is_none());
    }

    #[test]
    fn deck_pages_stay_grouped_and_standalone_files_are_loose() {
        // Stepping through a deck must not wander into the rest of the library,
        // which is the whole difference between presenting a document and
        // running an announcements loop.
        let db = crate::db::open_in_memory().unwrap();
        db.migrate().unwrap();
        db.add_media("/s/sermon/page-001.png", "Sermon · 1", "image", "Sermon").unwrap();
        db.add_media("/s/sermon/page-002.png", "Sermon · 2", "image", "Sermon").unwrap();
        db.add_media("/m/loose.png", "Loose", "image", "").unwrap();

        let rows = db.list_media().unwrap();
        let decks: Vec<String> = rows.iter().map(|(.., deck)| deck.clone()).collect();
        assert_eq!(decks, vec!["Sermon", "Sermon", ""]);
        assert_eq!(rows.iter().filter(|(.., d)| d == "Sermon").count(), 2);
    }
}
