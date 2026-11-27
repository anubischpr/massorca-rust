//! Where an account's credential may be sent (decision 0176): per kind, the one official https origin of its vendor.
//!
//! [`check_url`] is the caller's gate before any request that carries a credential from
//! [`Agents::with_account_credential`](crate::Agents::with_account_credential): the URL must be https and its origin
//! (scheme, host and port) must be exactly the kind's official one. The parse is deliberately strict, so it can never
//! read a host other than the one an HTTP client would connect to: anything but plain ASCII, a user part (`@`), a
//! backslash, a percent sign, an IPv6 literal or a trailing dot in the host is refused, not interpreted.
//!
//! The table is code, not kind data: a kind declared in a profile or a package never names where a credential goes.

/// The official origin of each kind whose credential may leave the machine. A kind not listed has none, and its
/// credential is never read for a vendor request.
const OFFICIAL_ORIGINS: &[(&str, &str)] = &[
    // Claude Code's OAuth credential is accepted by the Anthropic API, which serves its usage.
    ("claude", "https://api.anthropic.com"),
    // Codex's ChatGPT sign-in is accepted by the ChatGPT backend, which serves its usage and rate limits.
    ("codex", "https://chatgpt.com"),
];

/// Why a URL may not carry a kind's credential. The URL itself is never repeated: it may hold more than an origin.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EndpointRefused {
    /// The kind has no official vendor origin.
    #[error(
        "agent kind {kind} has no official vendor endpoint; its credential is never sent anywhere"
    )]
    NoOfficialOrigin { kind: String },
    /// The URL is not https.
    #[error("a credential is only ever sent over https")]
    NotHttps,
    /// The URL is not a plain absolute URL this check can read without doubt.
    #[error("the URL is not a plain absolute https URL")]
    Malformed,
    /// The URL's origin is not the kind's official one.
    #[error("the credential of agent kind {kind} is only ever sent to {official}")]
    OtherOrigin { kind: String, official: String },
}

/// The official origin of `kind`'s vendor, `https://host` with no path; `None` when its credential is never sent.
pub fn official_origin(kind: &str) -> Option<String> {
    #[cfg(any(test, feature = "test-hooks"))]
    if let Some(origin) = test_hooks::overridden(kind) {
        return Some(origin);
    }
    OFFICIAL_ORIGINS
        .iter()
        .find(|(id, _)| *id == kind)
        .map(|(_, origin)| (*origin).to_owned())
}

/// Accepts `url` only when it is https and its origin is exactly `kind`'s official origin.
pub fn check_url(kind: &str, url: &str) -> Result<(), EndpointRefused> {
    let official = official_origin(kind).ok_or_else(|| EndpointRefused::NoOfficialOrigin {
        kind: kind.to_owned(),
    })?;
    // The table and the test override hold only origins that parse; a broken entry refuses everything.
    let allowed = Origin::parse(&official).ok_or(EndpointRefused::Malformed)?;
    let scheme_end = url.find("://").ok_or(EndpointRefused::Malformed)?;
    let scheme = url.get(..scheme_end).unwrap_or_default();
    if !scheme.eq_ignore_ascii_case(allowed.scheme) {
        return Err(if allowed.scheme == "https" {
            EndpointRefused::NotHttps
        } else {
            EndpointRefused::OtherOrigin {
                kind: kind.to_owned(),
                official,
            }
        });
    }
    let origin = Origin::parse(url).ok_or(EndpointRefused::Malformed)?;
    if origin != allowed {
        return Err(EndpointRefused::OtherOrigin {
            kind: kind.to_owned(),
            official,
        });
    }
    Ok(())
}

/// A URL's scheme, lowercase host and port (the scheme's default when none is written).
#[derive(Debug, PartialEq, Eq)]
struct Origin {
    scheme: &'static str,
    host: String,
    port: u16,
}

impl Origin {
    /// The origin of `url`, or `None` when anything in it is outside the strict shape.
    fn parse(url: &str) -> Option<Self> {
        if !url.is_ascii()
            || url
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte == b' ' || byte == b'\\')
        {
            return None;
        }
        let (scheme, rest) = url.split_once("://")?;
        let (scheme, default_port) = if scheme.eq_ignore_ascii_case("https") {
            ("https", 443)
        } else if scheme.eq_ignore_ascii_case("http") {
            ("http", 80)
        } else {
            return None;
        };
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        if authority.contains(['@', '%', '[', ']']) {
            return None;
        }
        let (host, port) = match authority.split_once(':') {
            None => (authority, default_port),
            Some((host, port)) => {
                if port.is_empty() || port.len() > 5 || !port.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                let port: u16 = port.parse().ok()?;
                if port == 0 {
                    return None;
                }
                (host, port)
            }
        };
        let plain_label = |label: &str| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        };
        if host.is_empty() || !host.split('.').all(plain_label) {
            return None;
        }
        Some(Self {
            scheme,
            host: host.to_ascii_lowercase(),
            port,
        })
    }
}

/// Test-only fake vendor endpoints. Compiled only in this crate's unit tests and with the `test-hooks` feature, which
/// only dev-dependencies enable (`tests/test_hooks_gate.rs` fails a build where a normal dependency does). Even then an
/// override can only name a loopback origin, so it can never send a credential off the machine.
#[cfg(any(test, feature = "test-hooks"))]
pub mod test_hooks {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, PoisonError};

    use super::Origin;

    static OVERRIDES: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

    /// Gives `kind` the official origin `origin` until the returned guard is dropped. Tests running at once must use
    /// different kinds.
    ///
    /// # Panics
    ///
    /// When `origin` is not a plain `http` or `https` origin on `127.0.0.1` or `localhost`.
    pub fn override_origin(kind: &str, origin: &str) -> OriginOverride {
        let parsed = Origin::parse(origin).filter(|parsed| {
            matches!(parsed.host.as_str(), "127.0.0.1" | "localhost")
                && origin
                    .split_once("://")
                    .is_some_and(|(_, rest)| !rest.contains(['/', '?', '#']))
        });
        assert!(
            parsed.is_some(),
            "a test vendor origin must be a loopback origin with no path"
        );
        OVERRIDES
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(kind.to_owned(), origin.to_owned());
        OriginOverride {
            kind: kind.to_owned(),
        }
    }

    /// Removes its kind's override when dropped.
    #[derive(Debug)]
    pub struct OriginOverride {
        kind: String,
    }

    impl Drop for OriginOverride {
        fn drop(&mut self) {
            OVERRIDES
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&self.kind);
        }
    }

    pub(super) fn overridden(kind: &str) -> Option<String> {
        OVERRIDES
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(kind)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_credential_kind_has_an_https_origin_and_others_have_none() {
        assert_eq!(
            official_origin("claude").as_deref(),
            Some("https://api.anthropic.com")
        );
        assert_eq!(
            official_origin("codex").as_deref(),
            Some("https://chatgpt.com")
        );
        assert_eq!(official_origin("antigravity"), None);
        assert_eq!(official_origin("nope"), None);
        for (_, origin) in OFFICIAL_ORIGINS {
            assert_eq!(Origin::parse(origin).unwrap().scheme, "https");
        }
    }

    #[test]
    fn the_official_origin_is_accepted_with_any_path() {
        for url in [
            "https://api.anthropic.com",
            "https://api.anthropic.com/",
            "https://api.anthropic.com/api/oauth/usage",
            "https://api.anthropic.com:443/api/oauth/usage?x=1#y",
            "HTTPS://API.Anthropic.COM/api/oauth/usage",
            "https://api.anthropic.com/@elsewhere.example",
        ] {
            assert_eq!(check_url("claude", url), Ok(()), "{url}");
        }
        assert_eq!(
            check_url("codex", "https://chatgpt.com/backend-api/wham/usage"),
            Ok(())
        );
    }

    #[test]
    fn http_is_refused() {
        for url in [
            "http://api.anthropic.com/api/oauth/usage",
            "http://api.anthropic.com:443/",
            "ftp://api.anthropic.com/",
            "wss://api.anthropic.com/",
        ] {
            assert_eq!(
                check_url("claude", url),
                Err(EndpointRefused::NotHttps),
                "{url}"
            );
        }
    }

    #[test]
    fn other_and_lookalike_hosts_are_refused() {
        let other = || EndpointRefused::OtherOrigin {
            kind: "claude".to_owned(),
            official: "https://api.anthropic.com".to_owned(),
        };
        for url in [
            "https://example.com/",
            "https://chatgpt.com/",
            "https://api.anthropic.com.example.com/",
            "https://xapi.anthropic.com/",
            "https://anthropic.com/",
            "https://api-anthropic.com/",
            "https://api.anthropic.co/",
            "https://api.anthropic.com:8443/",
        ] {
            assert_eq!(check_url("claude", url), Err(other()), "{url}");
        }
        for url in [
            "https://api.anthropic.com@example.com/",
            "https://api.anthropic.com:443@example.com/",
            "https://user:pass@api.anthropic.com/",
            "https://api.anthropic.com\\@example.com/",
            "https://api.anthropic.com\\.example.com/",
            "https://api.anthropic.com./",
            "https://api.anthropic.com%2eexample.com/",
            "https://api.anthr\u{043e}pic.com/",
            "https://api.anthropic.com\t/",
            "https://[::1]/",
            "https://api.anthropic.com:/",
            "https://api.anthropic.com:0/",
            "https://api.anthropic.com:65536/",
            "https:///api.anthropic.com/",
            "https:/api.anthropic.com/",
            "api.anthropic.com/api/oauth/usage",
            "//api.anthropic.com/",
            "",
        ] {
            assert_eq!(
                check_url("claude", url),
                Err(EndpointRefused::Malformed),
                "{url:?}"
            );
        }
    }

    #[test]
    fn a_kind_without_an_official_origin_refuses_every_url() {
        assert_eq!(
            check_url("antigravity", "https://api.anthropic.com/"),
            Err(EndpointRefused::NoOfficialOrigin {
                kind: "antigravity".to_owned()
            })
        );
    }

    #[test]
    fn the_refusal_never_repeats_the_url() {
        let url = "https://example.com/?token=fake-secret";
        let message = check_url("claude", url).unwrap_err().to_string();
        assert!(!message.contains("fake-secret"), "{message}");
        assert!(!message.contains("example.com"), "{message}");
    }

    #[test]
    fn a_test_override_points_a_kind_at_a_fake_loopback_endpoint_only_while_held() {
        let kind = "fake-endpoint-override";
        assert_eq!(official_origin(kind), None);
        {
            let _fake = test_hooks::override_origin(kind, "http://127.0.0.1:9");
            assert_eq!(check_url(kind, "http://127.0.0.1:9/usage"), Ok(()));
            assert!(matches!(
                check_url(kind, "http://127.0.0.1:10/usage"),
                Err(EndpointRefused::OtherOrigin { .. })
            ));
            assert!(matches!(
                check_url(kind, "https://api.anthropic.com/"),
                Err(EndpointRefused::OtherOrigin { .. })
            ));
        }
        assert_eq!(official_origin(kind), None);
    }

    #[test]
    fn a_test_override_never_names_a_remote_origin() {
        for origin in [
            "https://example.com",
            "https://api.anthropic.com",
            "http://127.0.0.1:9/path",
            "http://127.0.0.1.example.com",
        ] {
            let caught = std::panic::catch_unwind(|| {
                test_hooks::override_origin("fake-endpoint-remote", origin)
            });
            assert!(caught.is_err(), "{origin}");
        }
        assert_eq!(official_origin("fake-endpoint-remote"), None);
    }
}
