use crate::commands::{
    navigate_handle, present_parallel_ref_handle, present_reference_handle, project_via_handle,
    AppState,
};
use crate::events::ProjectionState;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Manager};

pub const REMOTE_PORT: u16 = 8787;

/// How long a phone-sent alert stays up before dismissing itself.
const ALERT_SECONDS: i64 = 12;

static REMOTE_PAGE: OnceLock<String> = OnceLock::new();
static PROJECTION_PAGE: OnceLock<String> = OnceLock::new();

/// Probe targets, in the order their answers are offered to the operator.
///
/// A UDP `connect` sends nothing; it only asks the OS which local address it
/// *would* use to reach that target. A route to a directly connected subnet
/// beats the default route, so probing one address inside each common range
/// reveals the machine's address on that network.
///
/// 8.8.8.8 stands for "wherever the default route goes" and comes first because
/// on an ordinary church Wi-Fi that is the right answer. The rest matter when it
/// is not: a phone hotspot carrying no internet does not own the default route,
/// so the old single-probe approach handed back an address from some other
/// adapter and the phone had nothing to connect to.
struct Probe {
    /// The address to ask the routing table about.
    target: &'static str,
    /// May this machine legitimately *hold* that address? True only where
    /// holding it means "I am the hotspot", which is precisely the case a phone
    /// can reach. False for router addresses somebody else owns.
    hosted_here: bool,
}

const PROBES: &[Probe] = &[
    // Default route (ordinary Wi-Fi / Ethernet).
    Probe { target: "8.8.8.8:80", hosted_here: false },
    // Phone hotspots: the phone owns the gateway address, never this machine.
    Probe { target: "172.20.10.1:80", hosted_here: false },
    Probe { target: "192.168.43.1:80", hosted_here: false },
    // Windows mobile hotspot: this machine *is* the gateway when it is hosting.
    Probe { target: "192.168.137.1:80", hosted_here: true },
    // Common router addresses, owned by the router.
    Probe { target: "192.168.0.1:80", hosted_here: false },
    Probe { target: "192.168.1.1:80", hosted_here: false },
    Probe { target: "192.168.2.1:80", hosted_here: false },
    Probe { target: "10.0.0.1:80", hosted_here: false },
    Probe { target: "172.16.0.1:80", hosted_here: false },
];

/// Is a probe's answer an address worth handing the operator?
///
/// Loopback and link-local are never reachable from a phone. Neither is an
/// answer equal to the target itself: that means an adapter on this machine is
/// wearing a router's address (a static 192.168.1.1 on a spare NIC, a virtual
/// switch), so the OS matched it locally and no route to any phone exists. That
/// echo is what put an address in front of the operator that nothing could ever
/// connect to. A hotspot this machine hosts is the deliberate exception.
fn usable_answer(target_host: &str, ip: &str, hosted_here: bool) -> bool {
    if ip == "127.0.0.1" || ip == "0.0.0.0" || ip.starts_with("169.254.") {
        return false;
    }
    hosted_here || ip != target_host
}

fn host_of(target: &str) -> &str {
    target.split(':').next().unwrap_or(target)
}

fn probe(p: &Probe) -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect(p.target).ok()?;
    let addr: SocketAddr = socket.local_addr().ok()?;
    let ip = addr.ip();
    if ip.is_loopback() {
        return None;
    }
    let ip = ip.to_string();
    usable_answer(host_of(p.target), &ip, p.hosted_here).then_some(ip)
}

/// Every address a phone might reach this machine on, best guess first, with
/// duplicates removed. Empty only when the machine has no usable network.
fn lan_ips() -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for p in PROBES {
        if let Some(ip) = probe(p) {
            if !found.contains(&ip) {
                found.push(ip);
            }
        }
    }
    found
}

fn projection_json(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    // Bound to a local so the guard drops before `state` does.
    let json = match state.current.lock() {
        Ok(ref g) => serde_json::to_string(&**g).unwrap_or_else(|_| "{\"kind\":\"blank\"}".into()),
        Err(_) => "{\"kind\":\"blank\"}".into(),
    };
    json
}

fn describe(state: &ProjectionState) -> String {
    match state {
        ProjectionState::Verse { caption, .. } | ProjectionState::Song { caption, .. } => {
            format!("On screen: {caption}")
        }
        ProjectionState::Parallel { caption, primary_code, secondary_code, .. } => {
            format!("On screen: {caption} ({primary_code} / {secondary_code})")
        }
        ProjectionState::Image { .. } => "Image".into(),
        ProjectionState::Message { text } => format!("Message: {text}"),
        ProjectionState::Countdown { label, .. } => format!("Countdown: {label}"),
        ProjectionState::Logo => "Logo".into(),
        ProjectionState::Blackout => "Blackout".into(),
        ProjectionState::Blank => "Nothing (blank)".into(),
    }
}

/// Summary plus the listening flag. The phone takes the laptop's word for whether
/// it is listening, so reloading the page can never leave the button lying.
fn state_json(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let summary = match state.current.lock() {
        Ok(ref g) => describe(g),
        Err(_) => "…".into(),
    };
    let listening = state.listening.load(Ordering::SeqCst);
    serde_json::json!({ "summary": summary, "listening": listening }).to_string()
}

/// What the projection *looks* like: active theme plus font scale. The mirror
/// polls this so a phone or an OBS browser source shows the sepia the operator
/// chose, not a hardcoded black screen.
fn appearance_json(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    // Bound to a local so the guard drops before `state` does.
    let json = match state.settings.lock() {
        Ok(ref s) => serde_json::to_string(&**s).unwrap_or_else(|_| "{}".into()),
        Err(_) => "{}".into(),
    };
    json
}

/// Installed translations plus the active one, so the phone can offer every
/// other translation to compare against, exactly as the console does.
fn translations_json(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let active = state.active_translation();
    let list = crate::commands::list_translations(state).unwrap_or_default();
    serde_json::json!({ "active": active, "list": list }).to_string()
}

fn search_json(app: &AppHandle, query: &str) -> String {
    let state = app.state::<AppState>();
    let hits = crate::commands::search_scripture(query.to_string(), state).unwrap_or_default();
    let brief: Vec<_> = hits
        .into_iter()
        .take(12)
        .map(|v| {
            // The phone shows a one-line preview; sending whole verses over a
            // hall's Wi-Fi for a list of 12 is waste.
            let mut text = v.text;
            if text.chars().count() > 90 {
                text = text.chars().take(90).collect::<String>() + "…";
            }
            serde_json::json!({ "reference": v.reference, "text": text })
        })
        .collect();
    serde_json::to_string(&brief).unwrap_or_else(|_| "[]".into())
}

/// Start the LAN remote HTTP server on a background thread (idempotent).
///
/// Returns every address a phone might reach it on, best guess first. The server
/// binds `0.0.0.0`, so it is genuinely listening on all of them; the only real
/// question is which one the phone shares a network with, and the operator can
/// see that far better than we can guess it. Calling this again re-reads the
/// addresses, so switching to a hotspot mid-setup gives fresh ones.
pub fn start(app: AppHandle, running: Arc<AtomicBool>) -> Result<Vec<String>, String> {
    let mut ips = lan_ips();
    if ips.is_empty() {
        ips.push("127.0.0.1".into());
    }
    let urls: Vec<String> = ips.iter().map(|ip| format!("http://{ip}:{REMOTE_PORT}")).collect();
    if running.swap(true, Ordering::SeqCst) {
        return Ok(urls); // already running
    }

    let server = tiny_http::Server::http(("0.0.0.0", REMOTE_PORT))
        .map_err(|e| format!("could not start remote server: {e}"))?;

    std::thread::spawn(move || {
        for mut req in server.incoming_requests() {
            let path = req.url().split('?').next().unwrap_or("/").to_string();
            let method = req.method().to_string();

            let mut payload = String::new();
            if method == "POST" {
                let _ = std::io::Read::read_to_string(&mut req.as_reader(), &mut payload);
            }
            let (code, body) = route(&app, &method, &path, payload.trim());

            let mime = if path == "/" || path == "/projection" {
                "text/html; charset=utf-8"
            } else if matches!(
                path.as_str(),
                "/api/projection" | "/api/appearance" | "/api/translations" | "/api/state" | "/api/search"
            ) {
                "application/json; charset=utf-8"
            } else {
                "text/plain; charset=utf-8"
            };
            let response = tiny_http::Response::from_string(body)
                .with_status_code(code)
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).unwrap(),
                );
            let _ = req.respond(response);
        }
    });

    Ok(urls)
}

fn route(app: &AppHandle, method: &str, path: &str, body: &str) -> (u16, String) {
    match (method, path) {
        ("GET", "/") => (
            200,
            REMOTE_PAGE.get_or_init(crate::remote_pages::remote_page).clone(),
        ),
        ("GET", "/projection") => (
            200,
            PROJECTION_PAGE.get_or_init(crate::remote_pages::projection_page).clone(),
        ),
        ("GET", "/api/projection") => (200, projection_json(app)),
        ("GET", "/api/appearance") => (200, appearance_json(app)),
        ("GET", "/api/translations") => (200, translations_json(app)),
        ("GET", "/api/state") => (200, state_json(app)),

        // The operator is at the projector when the preacher steps up, not at
        // the laptop. Listening starts and stops from the phone too.
        ("POST", "/api/listen") => {
            let on = body == "start";
            let result = if on {
                crate::commands::begin_listening(app, None)
            } else {
                app.state::<AppState>().listening.store(false, Ordering::SeqCst);
                Ok(())
            };
            match result {
                Ok(()) => (200, if on { "listening" } else { "stopped" }.to_string()),
                Err(e) => (500, e),
            }
        }

        // Following a preacher through a passage is the common case; typing a
        // fresh reference for every verse is not.
        ("POST", "/api/nav") => match body {
            "next-verse" | "prev-verse" | "next-chapter" | "prev-chapter" => {
                match navigate_handle(app, body, crate::commands::BY_REMOTE) {
                    Some(v) => (200, v.reference),
                    None => (409, "Nothing on screen to move from.".into()),
                }
            }
            _ => (400, "Unknown direction.".into()),
        },

        ("POST", "/api/display") => {
            let next = match body {
                "blank" => ProjectionState::Blank,
                "blackout" => ProjectionState::Blackout,
                "logo" => ProjectionState::Logo,
                _ => return (400, "Unknown screen mode.".into()),
            };
            let _ = project_via_handle(app, next);
            (200, "ok".into())
        }

        // Kept for older phones that still have the previous page cached.
        ("POST", "/api/blank") => {
            let _ = project_via_handle(app, ProjectionState::Blank);
            (200, "ok".into())
        }

        ("POST", "/api/search") => {
            if body.is_empty() {
                return (400, "Nothing to search for.".into());
            }
            (200, search_json(app, body))
        }

        ("POST", "/api/alert") => {
            let state = app.state::<AppState>();
            let result = if body.is_empty() {
                crate::commands::clear_alert(app.clone(), state)
            } else {
                crate::commands::show_alert(app.clone(), state, body.to_string(), ALERT_SECONDS)
            };
            match result {
                Ok(()) => (200, "ok".into()),
                Err(e) => (500, e),
            }
        }

        // Goes through the same path as presenting at the laptop, so the cursor
        // and the desktop's "Presenting" follow the phone. Otherwise the next
        // tap on next/previous steps from whatever was up before.
        ("POST", "/api/project") => match present_reference_handle(app, body) {
            Ok(v) => (200, v.reference),
            Err(e) => (400, e),
        },

        // "<code>|<reference>". The reference is optional: left empty it means
        // the verse already on screen, which is what the operator wants after
        // stepping through a passage.
        ("POST", "/api/parallel") => {
            let (code, reference) = body.split_once('|').unwrap_or((body, ""));
            let (code, reference) = (code.trim(), reference.trim());
            if code.is_empty() {
                return (400, "Pick a translation to compare with.".into());
            }
            match present_parallel_ref_handle(app, code, reference) {
                Ok(v) => (200, v.reference),
                Err(e) => (400, e),
            }
        }

        _ => (404, "not found".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_iphone_hotspot_range_is_probed() {
        // The case that sent the operator an unreachable address: a hotspot with
        // no internet does not hold the default route, so it has to be asked for
        // by name.
        assert!(PROBES.iter().any(|p| p.target.starts_with("172.20.10.1")));
        assert_eq!(PROBES[0].target, "8.8.8.8:80", "the default route stays the first guess");
    }

    #[test]
    fn a_router_address_this_machine_wears_itself_is_not_offered() {
        // Measured on a laptop with a static 192.168.1.1 on a spare NIC: probing
        // the common router address answered with the machine's own, and that
        // address was offered to the operator. No phone shares a network with a
        // wired NIC, so it never connected. Rejecting the echo drops it.
        assert!(!usable_answer("192.168.1.1", "192.168.1.1", false));

        // A genuine 192.168.1.x network is untouched: there the router owns .1
        // and we answer with our own address on that network.
        assert!(usable_answer("192.168.1.1", "192.168.1.42", false));

        // Hosting a Windows mobile hotspot means holding the gateway address, and
        // that is exactly the address phones on it need.
        assert!(usable_answer("192.168.137.1", "192.168.137.1", true));

        // A phone's hotspot gateway belongs to the phone, so an echo there is
        // the same kind of local match, not a reachable address.
        assert!(!usable_answer("172.20.10.1", "172.20.10.1", false));

        // Unchanged rejections.
        assert!(!usable_answer("8.8.8.8", "169.254.80.158", false));
        assert!(!usable_answer("8.8.8.8", "0.0.0.0", false));
        assert!(!usable_answer("8.8.8.8", "127.0.0.1", false));
        assert!(usable_answer("8.8.8.8", "172.20.10.3", false));
    }

    #[test]
    fn addresses_are_unique_and_reachable_looking() {
        let ips = lan_ips();
        let mut seen = ips.clone();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), ips.len(), "duplicate addresses were offered");
        for ip in &ips {
            assert!(!ip.starts_with("127."), "loopback is useless to a phone: {ip}");
            assert!(!ip.starts_with("169.254."), "link-local is useless to a phone: {ip}");
            assert_ne!(ip, "0.0.0.0");
        }
    }

    #[test]
    fn probing_an_unroutable_target_never_yields_loopback() {
        // Whatever the OS answers, loopback must be filtered rather than shown.
        if let Some(ip) = probe(&Probe { target: "172.20.10.1:80", hosted_here: false }) {
            assert!(!ip.starts_with("127."));
        }
    }
}
