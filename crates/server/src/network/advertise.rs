//! Addresses a pairing offer lists in place of the bound listeners (SRV-07): what the caller typed, checked and written
//! as the `host:port` a device dials.

use std::net::{IpAddr, Ipv6Addr};

/// Addresses one offer may list.
pub(crate) const MAX_ADDRESSES: usize = 8;
/// The longest address accepted, after its scheme is removed: a DNS name and a port.
const MAX_ADDRESS_LEN: usize = 260;
/// Schemes accepted in front of an address; the device always connects over TLS, so each one means the same.
const SCHEMES: [&str; 4] = ["wss://", "https://", "ws://", "http://"];

/// The addresses as `host:port`, in the order given and without repeats. `default_port` fills a missing port.
pub(crate) fn normalize_all(raw: &[String], default_port: u16) -> Result<Vec<String>, String> {
    if raw.len() > MAX_ADDRESSES {
        return Err(format!("an offer lists at most {MAX_ADDRESSES} addresses"));
    }
    let mut out: Vec<String> = Vec::with_capacity(raw.len());
    for value in raw {
        let address = normalize(value, default_port)?;
        if !out.contains(&address) {
            out.push(address);
        }
    }
    Ok(out)
}

/// One address as `host:port` (`[v6]:port` for IPv6), or why a device could not dial it. The message quotes the value
/// only when it is short and printable.
pub(crate) fn normalize(raw: &str, default_port: u16) -> Result<String, String> {
    let shown = shown(raw);
    let refuse = |why: &str| Err(format!("address {shown}: {why}"));
    let text = raw.trim();
    if text.is_empty() {
        return refuse("it is empty");
    }
    if text.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return refuse("it contains spaces or control characters");
    }
    let lower = text.to_ascii_lowercase();
    let body = SCHEMES
        .iter()
        .find(|scheme| lower.starts_with(*scheme))
        .map_or(text, |scheme| &text[scheme.len()..]);
    // A URL may end in one `/`; any other path, a query, a fragment or credentials cannot go in an offer.
    let body = body.strip_suffix('/').unwrap_or(body);
    if body.len() > MAX_ADDRESS_LEN {
        return refuse("it is too long");
    }
    if body.contains(['/', '?', '#', '@']) {
        return refuse("give a host and port only, without a path, query or user name");
    }
    let (host, port) = split_host_port(body)
        .ok_or_else(|| format!("address {shown}: it is not host or host:port"))?;
    let port = match port {
        None => default_port,
        Some(text) => match text.parse::<u16>() {
            Ok(0) => return refuse("port 0 cannot be dialled"),
            Ok(port) => port,
            Err(_) => return refuse("the port is not a number from 1 to 65535"),
        },
    };
    if port == 0 {
        return refuse("no port was given and the service has no listener port to use");
    }
    match host {
        Host::V6(ip) if ip.is_unspecified() => refuse("a wildcard address cannot be dialled"),
        Host::V6(ip) => Ok(format!("[{ip}]:{port}")),
        Host::Name(name) => match name.parse::<IpAddr>() {
            Ok(ip) if ip.is_unspecified() => refuse("a wildcard address cannot be dialled"),
            Ok(ip) => Ok(format!("{ip}:{port}")),
            Err(_) if is_host_name(name) => Ok(format!("{}:{port}", name.to_ascii_lowercase())),
            Err(_) => refuse("it is not a host name or an IP address"),
        },
    }
}

enum Host<'a> {
    V6(Ipv6Addr),
    Name(&'a str),
}

/// `[v6]`, `[v6]:port`, a bare IPv6 literal, `host` or `host:port`.
fn split_host_port(body: &str) -> Option<(Host<'_>, Option<&str>)> {
    if let Some(rest) = body.strip_prefix('[') {
        let (ip, after) = rest.split_once(']')?;
        let ip = ip.parse::<Ipv6Addr>().ok()?;
        let port = match after {
            "" => None,
            _ => Some(after.strip_prefix(':')?),
        };
        return Some((Host::V6(ip), port));
    }
    if body.matches(':').count() > 1 {
        return body.parse::<Ipv6Addr>().ok().map(|ip| (Host::V6(ip), None));
    }
    match body.split_once(':') {
        Some((host, port)) => Some((Host::Name(host), Some(port))),
        None => Some((Host::Name(body), None)),
    }
}

/// Letters, digits and hyphens in dot-separated labels of 1 to 63 characters, 253 at most in all.
fn is_host_name(name: &str) -> bool {
    let name = name.strip_suffix('.').unwrap_or(name);
    !name.is_empty()
        && name.len() <= 253
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

fn shown(raw: &str) -> String {
    if raw.len() <= 80 && raw.chars().all(|c| !c.is_control()) {
        format!("{raw:?}")
    } else {
        "given".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORT: u16 = 7347;

    fn ok(raw: &str) -> String {
        normalize(raw, PORT).unwrap_or_else(|why| panic!("{raw} refused: {why}"))
    }

    #[test]
    fn host_forms_become_host_and_port() {
        assert_eq!(ok("10.0.2.2"), "10.0.2.2:7347");
        assert_eq!(ok("100.64.0.10:9000"), "100.64.0.10:9000");
        assert_eq!(ok("VPS.example.ts.net"), "vps.example.ts.net:7347");
        assert_eq!(ok("vps.example.ts.net:443"), "vps.example.ts.net:443");
        assert_eq!(ok("fd7a:115c:a1e0::1"), "[fd7a:115c:a1e0::1]:7347");
        assert_eq!(ok("[fd7a:115c:a1e0::1]:9000"), "[fd7a:115c:a1e0::1]:9000");
        assert_eq!(ok("[fd7a:115c:a1e0::1]"), "[fd7a:115c:a1e0::1]:7347");
    }

    #[test]
    fn urls_lose_their_scheme_and_trailing_slash() {
        assert_eq!(
            ok("wss://vps.example.ts.net:9000"),
            "vps.example.ts.net:9000"
        );
        assert_eq!(ok("HTTPS://vps.example.ts.net/"), "vps.example.ts.net:7347");
        assert_eq!(ok("http://100.64.0.10:80"), "100.64.0.10:80");
    }

    #[test]
    fn undialable_addresses_are_refused() {
        for raw in [
            "",
            "0.0.0.0",
            "0.0.0.0:7347",
            "::",
            "[::]:7347",
            "*",
            "host:0",
            "host:99999",
            "host:port",
            "wss://host/path",
            "host?x=1",
            "host#frag",
            "user@host",
            "wss://user:pw@host",
            "bad host",
            "tab\there",
            "-bad-.example",
            "[fd7a::1]x",
        ] {
            assert!(normalize(raw, PORT).is_err(), "{raw:?} was accepted");
        }
        assert!(normalize(&"a".repeat(300), PORT).is_err());
    }

    #[test]
    fn no_port_and_no_listener_port_is_refused() {
        assert!(normalize("10.0.2.2", 0).is_err());
        assert_eq!(
            normalize("10.0.2.2:7000", 0).as_deref(),
            Ok("10.0.2.2:7000")
        );
    }

    #[test]
    fn a_list_keeps_order_drops_repeats_and_is_capped() {
        let raw = vec![
            "10.0.2.2".to_owned(),
            "10.0.2.2:7347".to_owned(),
            "localhost".to_owned(),
        ];
        assert_eq!(
            normalize_all(&raw, PORT).as_deref(),
            Ok(&["10.0.2.2:7347".to_owned(), "localhost:7347".to_owned()][..])
        );
        let many = vec!["10.0.2.2".to_owned(); MAX_ADDRESSES + 1];
        assert!(normalize_all(&many, PORT).is_err());
    }
}
