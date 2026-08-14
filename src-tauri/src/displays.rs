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
    fn a_deliberately_chosen_laptop_screen_is_still_honoured() {
        // Rehearsing with no TV connected: the operator can point output at
        // their own screen, and gets a window rather than a takeover.
        let displays = vec![display("Laptop", true)];
        let target = displays.iter().find(|d| d.name == "Laptop");
        assert!(!should_fill(target));
    }
}
