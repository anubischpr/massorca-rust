//! The URL a listening port is opened at (decision 0131): the last `http(s)` URL with that port that its terminal printed,
//! as dev servers print `Local: http://localhost:5173/`, else a guess from the socket's address.

/// The URL for `port`, and whether the terminal printed it. Only a printed URL on a loopback or wildcard host counts, the
/// last one printed; a URL on any other host (a dev server's `Network:` line, or anything a program prints) is ignored, so
/// the URL stays informational and never points away from the host.
pub(crate) fn advertised_url(screen: &str, port: u16, connect_host: &str) -> (String, bool) {
    let local = printed_urls(screen)
        .filter(|url| {
            authority(url).is_some_and(|(host, url_port)| url_port == port && is_local_host(host))
        })
        .last();
    match local {
        Some(url) => (without_query(url).to_owned(), true),
        None => (guessed_url(connect_host, port), false),
    }
}

/// A printed URL without its query and fragment, where dev servers put access tokens (Jupyter's `?token=`).
fn without_query(url: &str) -> &str {
    url.find(['?', '#']).map_or(url, |end| &url[..end])
}

/// The URL a socket is opened at when nothing printed one, or when the caller may not read what was printed.
pub(crate) fn guessed_url(connect_host: &str, port: u16) -> String {
    if connect_host.contains(':') {
        format!("http://[{connect_host}]:{port}/")
    } else {
        format!("http://{connect_host}:{port}/")
    }
}

/// Every `http://` and `https://` URL in `text`, up to the first space, quote or closing bracket, without trailing
/// sentence punctuation.
fn printed_urls(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        loop {
            let start = ["http://", "https://"]
                .iter()
                .filter_map(|scheme| rest.find(scheme))
                .min()?;
            let candidate = &rest[start..];
            let end = candidate
                .find(|c: char| {
                    c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | ')' | '`')
                })
                .unwrap_or(candidate.len());
            let url = candidate[..end].trim_end_matches(['.', ',', ';', ':']);
            rest = &candidate[end..];
            if url.len() > "https://".len() {
                return Some(url);
            }
        }
    })
}

/// The host and explicit port of a URL; `None` without a port.
fn authority(url: &str) -> Option<(&str, u16)> {
    let after_scheme = url.split_once("://")?.1;
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let (host, port) = authority.rsplit_once(':')?;
    if host.starts_with('[') != host.ends_with(']') {
        return None;
    }
    Some((
        host.trim_start_matches('[').trim_end_matches(']'),
        port.parse().ok()?,
    ))
}

/// `localhost`, or a loopback or wildcard address (127.0.0.0/8, `::1`, `0.0.0.0`, `::`).
fn is_local_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback() || address.is_unspecified())
}

#[cfg(test)]
mod tests {
    use super::advertised_url;

    #[test]
    fn a_printed_local_url_with_the_port_wins_over_a_network_one() {
        let screen = "  VITE ready\n  ➜  Local:   http://localhost:5173/app/\n  ➜  Network: http://192.168.1.5:5173/app/\n";
        assert_eq!(
            advertised_url(screen, 5173, "localhost"),
            ("http://localhost:5173/app/".to_owned(), true)
        );
    }

    #[test]
    fn the_scheme_path_and_last_print_are_kept_and_other_ports_ignored() {
        let screen = "listening on https://127.0.0.1:8443/old\nother http://localhost:3000/\nnow at https://127.0.0.1:8443/new.";
        assert_eq!(
            advertised_url(screen, 8443, "127.0.0.1"),
            ("https://127.0.0.1:8443/new".to_owned(), true)
        );
        let token =
            "Jupyter at http://127.0.0.1:8888/lab?token=abc#cell and http://localhost:9000#x";
        assert_eq!(
            advertised_url(token, 8888, "127.0.0.1"),
            ("http://127.0.0.1:8888/lab".to_owned(), true)
        );
        assert_eq!(
            advertised_url(token, 9000, "localhost"),
            ("http://localhost:9000".to_owned(), true)
        );
        // A URL on any other host is not trusted: the guess stands in for it.
        let network_only = "Network: http://10.0.0.2:4000 or https://evil.example:4000/login";
        assert_eq!(
            advertised_url(network_only, 4000, "localhost"),
            ("http://localhost:4000/".to_owned(), false)
        );
        for host in ["127.1.2.3", "[::1]", "0.0.0.0", "[::]"] {
            let screen = format!("at http://{host}:4000/x");
            assert_eq!(
                advertised_url(&screen, 4000, "localhost"),
                (format!("http://{host}:4000/x"), true)
            );
        }
    }

    #[test]
    fn without_a_printed_url_it_is_guessed_from_the_address() {
        assert_eq!(
            advertised_url(
                "no url here http://localhost/ (\"https://",
                3000,
                "localhost"
            ),
            ("http://localhost:3000/".to_owned(), false)
        );
        assert_eq!(
            advertised_url("", 3000, "::1"),
            ("http://[::1]:3000/".to_owned(), false)
        );
        assert_eq!(
            advertised_url("see http://[::1]:3000/x", 3000, "::1"),
            ("http://[::1]:3000/x".to_owned(), true)
        );
    }
}
