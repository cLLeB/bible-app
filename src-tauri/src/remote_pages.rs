//! The two pages the LAN server serves: the operator's phone remote, and the
//! read-only projection mirror used for OBS / browser output. This module holds
//! what they share and assembles each from its parts.
//!
//! Both poll the app for state. They do it with a *sequential* loop: each cycle
//! waits for the previous response before scheduling the next, and every request
//! carries an abort timeout. A naive `setInterval` fetch pile-up is what makes a
//! phone browser exhaust its per-host connection pool and leave a second tab
//! hanging forever, so the pattern here matters more than it looks.
//!
//! The pages are assembled by concatenation rather than `format!`. Both are
//! mostly CSS and JavaScript, and `format!` would demand every brace in them be
//! doubled, an easy way to ship subtly broken script that only misbehaves on a
//! phone. Plain `concat` keeps the source readable and removes the hazard.
//!
//! Each page is built once and cached: the markup never varies by request, and a
//! church laptop has better things to do than rebuild a page per poll.

use std::sync::OnceLock;

use crate::remote_control_js::{
    BROWSE_JS, MEDIA_JS, POLL_JS, SCREEN_JS, SONGS_JS, TABS_JS, WORD_JS,
};
use crate::remote_control_page::head_and_body;
use crate::remote_mirror_page::{PROJECTION_HTML, PROJECTION_JS, PROJECTION_THEME_JS};

/// Polling and timeout helpers. Shared: a stuck request must never hang a page
/// forever, whichever page it is.
const LOOP_JS: &str = r#"
async function timedFetch(path,opts){
 opts=opts||{};
 const c=new AbortController();
 const t=setTimeout(function(){c.abort()},6000);
 try{return await fetch(path,Object.assign({},opts,{signal:c.signal,cache:'no-store'}));}
 finally{clearTimeout(t);}
}
async function loop(fn,ms){for(;;){try{await fn()}catch(e){}await new Promise(function(r){setTimeout(r,ms)});}}
"#;

/// The control page's request helper. There is no pairing step: opening the
/// address is the whole of it, so this just forwards to the shared fetch. Kept as
/// its own function because every handler calls `req`.
const REQ_JS: &str = r#"
async function req(path,opts){
 return await timedFetch(path,opts||{});
}
"#;

static REMOTE_PAGE: OnceLock<String> = OnceLock::new();
static PROJECTION_PAGE: OnceLock<String> = OnceLock::new();

fn build_remote_page() -> String {
    [
        head_and_body().as_str(),
        LOOP_JS,
        REQ_JS,
        TABS_JS,
        WORD_JS,
        BROWSE_JS,
        SONGS_JS,
        MEDIA_JS,
        SCREEN_JS,
        POLL_JS,
    ]
    .concat()
}

pub fn remote_page() -> String {
    REMOTE_PAGE.get_or_init(build_remote_page).clone()
}

pub fn projection_page() -> String {
    PROJECTION_PAGE
        .get_or_init(|| {
            [PROJECTION_HTML, LOOP_JS, PROJECTION_THEME_JS, PROJECTION_JS].concat()
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_are_whole_documents() {
        for page in [remote_page(), projection_page()] {
            assert!(page.starts_with("<!doctype html>"));
            assert!(page.trim_end().ends_with("</html>"));
            assert_eq!(
                page.matches("<script>").count(),
                page.matches("</script>").count(),
                "unbalanced script tags"
            );
        }
    }

    #[test]
    fn both_pages_poll_sequentially_and_time_out() {
        // Without these a phone piles up overlapping requests and can exhaust its
        // per-host connection pool, which strands any other tab on the same host.
        for page in [remote_page(), projection_page()] {
            assert!(page.contains("async function loop("), "poll loop missing");
            assert!(page.contains("AbortController"), "request timeout missing");
        }
    }

    #[test]
    fn neither_page_asks_the_phone_to_prove_anything() {
        // Opening the address is the whole of it. Nothing is stored on the phone
        // and no header is demanded, so a fresh browser works on first load.
        for page in [remote_page(), projection_page()] {
            assert!(!page.contains("X-Remote-Key"));
            assert!(!page.contains("remote-key"));
            assert!(!page.contains("localStorage"));
            assert!(!page.contains("Pairing code"));
        }
    }

    #[test]
    fn every_control_the_remote_offers_has_a_handler() {
        let page = remote_page();
        for handler in [
            "function nav(",
            "function go(",
            "function disp(",
            "function search(",
            "function listen(",
            "function sendAlert(",
            "function clearAlert(",
            "function both(",
            "function toggleTheme(",
            "function slideshow(",
            "function playMedia(",
            "function showTab(",
            "function openSong(",
            "function closeSong(",
            "function slideStep(",
            "function projectSlide(",
            "function deck(",
            "function vid(",
            "function sendMessage(",
            "function clearMessage(",
            "function countdown(",
            "function sendNote(",
            "function clearNote(",
            "function stageTimer(",
            "function size(",
            "function useTranslation(",
            "function browseUp(",
            "function openBook(",
            "function openChapter(",
            "function projectVerse(",
        ] {
            assert!(page.contains(handler), "{handler} is wired to a button but not defined");
        }
    }

    /// Every `onclick="name(` in the markup must name a function the script
    /// actually defines. The list above catches a deleted handler; this catches
    /// a *typo'd* one, which is the failure that reaches a phone silently.
    #[test]
    fn no_button_calls_a_function_that_does_not_exist() {
        let page = remote_page();
        for (i, _) in page.match_indices("onclick=\"") {
            let rest = &page[i + "onclick=\"".len()..];
            let name = rest.split('(').next().unwrap_or("");
            assert!(
                page.contains(&format!("function {name}(")),
                "onclick calls {name}() which is never defined"
            );
        }
    }

    #[test]
    fn the_remote_reaches_every_part_of_a_service() {
        // The gaps this page was widened to close. Each is something the console
        // could already do and the operator standing in the hall could not.
        let page = remote_page();
        for (route, what) in [
            ("/api/songs", "the song list"),
            ("/api/song", "projecting a song slide"),
            ("/api/deck", "turning a deck's pages"),
            ("/api/video", "video transport"),
            ("/api/message", "a full-screen message"),
            ("/api/countdown", "a countdown"),
            ("/api/stage-note", "a note to the stage"),
            ("/api/stage-timer", "the stage timer"),
            ("/api/fontscale", "text size on the wall"),
            ("/api/translation", "changing translation"),
            ("/api/books", "the book list"),
            ("/api/count", "counting chapters and verses"),
        ] {
            assert!(page.contains(route), "{what} never reaches the app");
        }
    }

    #[test]
    fn the_bible_can_be_reached_by_tapping_rather_than_typing() {
        // Typing "1 Thessalonians 4:16" one-handed in a dark hall is the worst
        // input the app asks for. Book, then chapter, then verse must all be
        // grids, and only one of the three visible at a time.
        let page = remote_page();
        for grid in ["bookgrid", "chapgrid", "versegrid"] {
            assert!(page.contains(&format!("id=\"{grid}\"")), "no {grid} to tap through");
        }
        assert!(page.contains("function browseLevel("), "the three grids are never swapped");
        // Chapters and verses are only ever offered from what the app actually
        // holds, so a translation missing a book cannot offer its chapters.
        assert!(page.contains("/api/count?book="), "the counts are guessed rather than asked for");
        // Verses project through the same parser the typed box uses.
        assert!(page.contains("atBook.name+' '+atChapter+':'+v"), "browse builds its own reference");
    }

    #[test]
    fn the_tabs_and_the_sections_they_switch_line_up() {
        // A tab whose section is missing shows an empty page and no error, so the
        // two lists are checked against each other rather than by eye.
        let page = remote_page();
        for tab in ["word", "songs", "media", "screen"] {
            assert!(page.contains(&format!("id=\"tab-{tab}\"")), "no section for the {tab} tab");
            assert!(page.contains(&format!("data-tab=\"{tab}\"")), "no button for the {tab} tab");
            assert!(page.contains(&format!("showTab('{tab}')")), "the {tab} tab is unreachable");
        }
    }

    #[test]
    fn the_controls_for_live_media_start_hidden() {
        // Transport and page buttons are raised by the poll only when something
        // is on the wall to control. Shipped visible, they would offer to pause a
        // video that is not playing.
        let page = remote_page();
        assert!(page.contains("<div id=\"vid\" class=\"ctx\" hidden>"), "transport starts shown");
        assert!(page.contains("<div id=\"deck\" class=\"ctx\" hidden>"), "page buttons start shown");
        assert!(page.contains("function paintContext("), "nothing ever raises them");
    }

    #[test]
    fn the_remote_can_compare_translations() {
        // The console offers "Compare with … / Both" on a looked-up verse; the
        // operator holding the phone needs the same, not a walk back to the desk.
        let page = remote_page();
        assert!(page.contains("Compare with"), "the compare control is missing");
        assert!(page.contains("/api/translations"), "the picker is never filled");
        assert!(page.contains("/api/parallel"), "Both never reaches the app");
    }

    #[test]
    fn the_remote_reads_in_both_light_and_dark() {
        let page = remote_page();
        // Following the phone's own setting is the default; the button only
        // overrides it, and nothing is written to the phone to remember that.
        assert!(page.contains("prefers-color-scheme:dark"), "no light/dark following");
        assert!(page.contains("data-theme"), "the override has nothing to set");
        assert!(!page.contains("color-scheme:dark}"), "the page is still pinned to dark");
    }

    #[test]
    fn the_remote_keeps_clear_of_a_phones_own_furniture() {
        // A fixed bar at the bottom of an iPhone lands under the home indicator,
        // and the pinned header under the notch, unless both are inset.
        let page = remote_page();
        assert!(page.contains("viewport-fit=cover"), "the insets are never reported");
        assert!(page.contains("env(safe-area-inset-bottom)"), "the tab bar ignores the inset");
        assert!(page.contains("env(safe-area-inset-top)"), "the header ignores the inset");
    }

    #[test]
    fn the_mirror_wears_the_operators_theme() {
        // A sepia service mirrored as white-on-black is the wrong screen, both on
        // a phone and in an OBS scene.
        let page = projection_page();
        assert!(page.contains("/api/appearance"), "the mirror never asks for the theme");
        assert!(page.contains("function applyTheme("), "the theme is fetched but not applied");
        assert!(page.contains("captionColor"), "the caption keeps a hardcoded colour");
        assert!(page.contains("linear-gradient("), "gradient themes are not mirrored");
    }

    #[test]
    fn the_remote_starts_polling_without_a_gate_in_front_of_it() {
        let page = remote_page();
        assert!(page.contains("\nstart();"), "the page must start polling on load");
        assert!(!page.contains("function boot("), "the pairing gate is gone");
    }

    #[test]
    fn the_remote_uses_helpers_it_actually_defines() {
        let page = remote_page();
        assert!(page.contains("async function req("), "req() is called but never defined");
        assert!(page.contains("async function timedFetch("), "timedFetch() missing");
    }
}
