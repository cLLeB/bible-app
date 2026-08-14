//! Where the congregation screen goes, and how it behaves once it is there.
//!
//! A church plugs a TV into HDMI and expects the words to appear on the TV
//! while the operator keeps working on the laptop. Two things have to be true
//! for that, and neither is automatic:
//!
//! **The right screen has to be chosen.** Enumeration order is not a reliable
//! answer: index 1 is whichever display the OS happened to list second, which
//! on some machines is the laptop panel. The question being asked is "which
//! screen is not the one the operator is looking at", so that is the question
//! this asks.
//!
//! **The operator's keyboard has to stay put.** Focusing the output window on
//! every cue steals the caret out of the search box mid-service and pulls the
//! window onto whatever virtual desktop the operator is using. Output is
//! something to be looked at, never something to be given focus.

use serde::{Deserialize, Serialize};

/// One screen the OS is offering, in the terms this app cares about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    /// The OS's name for it, used to remember a choice across restarts.
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// The screen the desktop and taskbar live on: the operator's own.
    pub primary: bool,
}

/// What the operator wants the congregation screen to use.
#[derive(Debug, Clone, PartialEq)]
pub enum Choice<'a> {
    /// Put it on the screen the operator is not using. The default, and right
    /// for the ordinary case of one laptop and one TV.
    Automatic,
    /// A specific screen, remembered by name.
    Named(&'a str),
}

/// Where the output window should go, or None when there is nowhere sensible.
///
/// None is a real answer, not a failure: on a single-screen laptop there is no
/// second screen, and covering the console with a fullscreen window the
/// operator then has to alt-tab out of is worse than showing them a window.
pub fn choose<'a>(displays: &'a [DisplayInfo], want: Choice<'_>) -> Option<&'a DisplayInfo> {
    if displays.is_empty() {
        return None;
    }
    if let Choice::Named(name) = want {
        // A remembered screen that is currently unplugged falls through to the
        // automatic answer rather than leaving the output on a screen that is
        // no longer there.
        if let Some(found) = displays.iter().find(|d| d.name == name) {
            return Some(found);
        }
    }
    // The first screen that is not the operator's own.
    displays.iter().find(|d| !d.primary)
}

/// Should the output window take over the whole screen?
///
/// Only when it has a screen of its own. Fullscreen on the operator's only
/// display hides the console behind the very thing they are trying to drive.
pub fn should_fill(target: Option<&DisplayInfo>) -> bool {
    matches!(target, Some(display) if !display.primary)
}

/// A fingerprint of the current screens: what they are, where, and how big.
///
/// Comparing this is how a TV being plugged in is noticed. Name alone is not
/// enough, because a resolution change moves the output window's whole
/// coordinate space without any screen appearing or disappearing.
pub fn signature(displays: &[DisplayInfo]) -> String {
    displays
        .iter()
        .map(|d| format!("{}@{},{};{}x{}", d.name, d.x, d.y, d.width, d.height))
        .collect::<Vec<_>>()
        .join("|")
}

/// How often to look. Plugging in a TV is a physical act nobody does twice a
/// second, and enumerating a handful of monitors is far cheaper than the poll
/// interval implies.
const WATCH_EVERY: std::time::Duration = std::time::Duration::from_secs(2);

/// Notice screens coming and going, and follow them.
///
/// There is no cross-platform display-change event to subscribe to, so this
/// asks. The alternative is what every other church app does, which is to make
/// the operator find a Refresh button at the moment they are least able to look
/// for one: the TV has just been plugged in and the service is starting.
///
/// Output is only re-placed when it is already showing. Noticing a screen must
/// never be a reason to put something in front of a congregation.
pub fn watch(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut last = signature(&crate::commands::displays_now(&app));
        loop {
            std::thread::sleep(WATCH_EVERY);
            let displays = crate::commands::displays_now(&app);
            let now = signature(&displays);
            if now == last {
                continue;
            }
            last = now;
            let _ = tauri::Emitter::emit(&app, "displays-changed", &displays);
            let showing = tauri::Manager::get_webview_window(&app, "projection")
                .and_then(|w| w.is_visible().ok())
                .unwrap_or(false);
            if showing {
                let _ = crate::commands::place_projection_window(&app);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(name: &str, primary: bool) -> DisplayInfo {
        DisplayInfo { name: name.into(), x: 0, y: 0, width: 1920, height: 1080, primary }
    }

    #[test]
    fn the_tv_is_chosen_over_the_laptop_whatever_the_order() {
        // The old code took whichever screen the OS listed second. Here the TV
        // is listed first, and index-based picking would have put the words on
        // the operator's own screen.
        let displays = vec![display("HDMI TV", false), display("Laptop", true)];
        assert_eq!(choose(&displays, Choice::Automatic).unwrap().name, "HDMI TV");

        let other_order = vec![display("Laptop", true), display("HDMI TV", false)];
        assert_eq!(choose(&other_order, Choice::Automatic).unwrap().name, "HDMI TV");
    }

    #[test]
    fn a_remembered_screen_wins_when_it_is_plugged_in() {
        let displays =
            vec![display("Laptop", true), display("Foyer TV", false), display("Main TV", false)];
        assert_eq!(choose(&displays, Choice::Named("Main TV")).unwrap().name, "Main TV");
        // Automatic would have taken the other one, so the choice is doing work.
        assert_eq!(choose(&displays, Choice::Automatic).unwrap().name, "Foyer TV");
    }

    #[test]
    fn a_remembered_screen_that_is_unplugged_falls_back() {
        // Someone unplugs the TV named last week and plugs in a different one.
        // Falling back beats projecting onto a screen that is not there.
        let displays = vec![display("Laptop", true), display("Borrowed TV", false)];
        assert_eq!(choose(&displays, Choice::Named("Main TV")).unwrap().name, "Borrowed TV");
    }

    #[test]
    fn one_screen_means_nowhere_to_put_it() {
        // Not a failure: it is why the window must not go fullscreen and bury
        // the console the operator is working in.
        let displays = vec![display("Laptop", true)];
        assert_eq!(choose(&displays, Choice::Automatic), None);
        assert_eq!(choose(&displays, Choice::Named("Main TV")), None);
        assert_eq!(choose(&[], Choice::Automatic), None);
    }

    #[test]
    fn the_operators_own_screen_is_never_taken_over() {
        let laptop = display("Laptop", true);
        let tv = display("Main TV", false);
        assert!(should_fill(Some(&tv)));
        assert!(!should_fill(Some(&laptop)), "fullscreen would bury the console");
        assert!(!should_fill(None));
    }

    #[test]
    fn plugging_a_tv_in_changes_the_fingerprint() {
        let laptop = vec![display("Laptop", true)];
        let with_tv = vec![display("Laptop", true), display("HDMI TV", false)];
        assert_ne!(signature(&laptop), signature(&with_tv));
        // Unplugging is the same event in reverse, and must be noticed too.
        assert_eq!(signature(&laptop), signature(&[display("Laptop", true)]));
    }

    #[test]
    fn a_resolution_change_counts_as_a_change() {
        // The screens are the same screens, but the output window's coordinate
        // space has moved under it, so its position is now wrong.
        let before = vec![display("HDMI TV", false)];
        let mut after = before.clone();
        after[0].width = 3840;
        after[0].height = 2160;
        assert_ne!(signature(&before), signature(&after));
    }

    #[test]
    fn nothing_connected_is_a_stable_fingerprint_not_a_wobble() {
        // An empty answer during a mode switch must not read as a change on
        // every poll, or output would be re-placed continuously.
        assert_eq!(signature(&[]), signature(&[]));
    }

    #[test]
    fn a_deliberately_chosen_laptop_screen_is_still_honoured() {
        // Rehearsing with no TV connected: the operator can point output at
        // their own screen, and gets a window rather than a takeover.
        let displays = vec![display("Laptop", true)];
        let target = displays.iter().find(|d| d.name == "Laptop");
        assert!(!should_fill(target));
    }
}
