//! The LAN remote's network side: working out which address a phone can reach
//! this machine on, and running the little HTTP server. What each request
//! *means* lives in `remote_api`; this module only gets it there and back.

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;

pub const REMOTE_PORT: u16 = 8787;

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
            let url = req.url().to_string();
            let (path, query) = url.split_once('?').unwrap_or((url.as_str(), ""));
            let (path, query) = (path.to_string(), query.to_string());
            let method = req.method().to_string();

            let mut payload = String::new();
            if method == "POST" {
                let _ = std::io::Read::read_to_string(&mut req.as_reader(), &mut payload);
            }
            let (code, body) =
                crate::remote_api::route(&app, &method, &path, &query, payload.trim());

            let mime = if path == "/" || path == "/projection" {
                "text/html; charset=utf-8"
            } else if crate::remote_api::is_json_path(&path) {
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

    #[test]
    fn a_url_splits_into_a_path_and_a_query_the_way_the_server_reads_it() {
        // `/api/song?id=7` has to reach the router as ("/api/song", "id=7"), or
        // the song route looks up an id it was never given.
        for (url, path, query) in [
            ("/api/song?id=7", "/api/song", "id=7"),
            ("/api/state", "/api/state", ""),
            ("/", "/", ""),
            ("/api/song?", "/api/song", ""),
        ] {
            assert_eq!(url.split_once('?').unwrap_or((url, "")), (path, query));
        }
    }
}
