//! PJLink projector control (Class 1) over TCP, port 4352. Lets the operator
//! power a network projector on/off, blank/unblank it (AV mute), or query its
//! state from inside the app. Fully local — the projector is on the church LAN.
//!
//! Protocol: on connect the projector sends a greeting line, either
//! `PJLINK 0` (no auth) or `PJLINK 1 <seed>` (auth). For auth, every command is
//! prefixed with the lowercase-hex MD5 of `seed + password`. Commands look like
//! `%1POWR 1\r`; the projector answers `%1POWR=OK` (or `=ERR3`, etc.).

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use md5::{Digest, Md5};

const TIMEOUT: Duration = Duration::from_secs(4);

/// Build the bytes to send: with auth, the md5 of `seed + password` (lowercase
/// hex) is prepended to the command body; without auth, just the body. A CR
/// terminates the line.
pub fn authed_command(seed: Option<&str>, password: &str, body: &str) -> String {
    match seed {
        Some(seed) => {
            let mut h = Md5::new();
            h.update(seed.as_bytes());
            h.update(password.as_bytes());
            let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
            format!("{hex}{body}\r")
        }
        None => format!("{body}\r"),
    }
}

/// Parse the greeting: `PJLINK 0` → no auth, `PJLINK 1 <seed>` → auth seed.
pub fn parse_seed(line: &str) -> Result<Option<String>, String> {
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    match parts.as_slice() {
        ["PJLINK", "0"] => Ok(None),
        ["PJLINK", "1", seed] => Ok(Some((*seed).to_string())),
        _ => Err(format!("unexpected PJLink greeting: {}", line.trim())),
    }
}

/// Connect, authenticate if required, send one command body (e.g. "%1POWR 1"),
/// and return the projector's trimmed response line.
pub fn send(host: &str, port: u16, password: &str, body: &str) -> Result<String, String> {
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("resolve {host}:{port}: {e}"))?
        .next()
        .ok_or_else(|| format!("no address for {host}:{port}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(TIMEOUT)).ok();
    stream.set_write_timeout(Some(TIMEOUT)).ok();

    let mut buf = [0u8; 256];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let greeting = String::from_utf8_lossy(&buf[..n]).to_string();
    // The greeting may arrive on the same read as nothing else; take its first line.
    let greeting_line = greeting.lines().next().unwrap_or("");
    let seed = parse_seed(greeting_line)?;

    let cmd = authed_command(seed.as_deref(), password, body);
    stream.write_all(cmd.as_bytes()).map_err(|e| e.to_string())?;

    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let resp = String::from_utf8_lossy(&buf[..n]).trim().to_string();
    if resp.contains("PJLINK ERRA") {
        return Err("authentication failed (wrong password)".into());
    }
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_auth_command_is_body_plus_cr() {
        assert_eq!(authed_command(None, "pw", "%1POWR 1"), "%1POWR 1\r");
    }

    #[test]
    fn auth_command_prefixes_the_spec_digest() {
        // From the PJLink spec: seed 498e4a67 + password JBMIAProjectorLink
        // → MD5 5d8409bc1c3fa39749434aa3a5c38682.
        let out = authed_command(Some("498e4a67"), "JBMIAProjectorLink", "%1POWR 1");
        assert_eq!(out, "5d8409bc1c3fa39749434aa3a5c38682%1POWR 1\r");
    }

    #[test]
    fn parses_greetings() {
        assert_eq!(parse_seed("PJLINK 0").unwrap(), None);
        assert_eq!(parse_seed("PJLINK 1 498e4a67").unwrap(), Some("498e4a67".into()));
        assert!(parse_seed("garbage").is_err());
    }
}
