//! Keeps secret-looking values out of every local log (service and desktop). A `key=value` or `key: value` whose key
//! names a secret is written with the value replaced by `[redacted]`: a bare value, a quoted one (even inside a
//! Debug-formatted string), or the credential after an `authorization` scheme or a `Bearer` word. A URL's `user:pass@`
//! password, a `?key=` or `&sig=` value, and a token with a known secret prefix (`sk-`, `ghp_`, a pairing offer) are
//! redacted too.

use std::fmt;

use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

/// What a secret-looking value is written as.
pub const REDACTED: &str = "[redacted]";

/// Keys whose value is redacted wherever they appear, in any case; word characters may follow them (`client_secret`).
const SECRET_KEYS: [&str; 15] = [
    "password",
    "passwd",
    "pwd",
    "passphrase",
    "secret",
    "token",
    "api_key",
    "api-key",
    "apikey",
    "private_key",
    "privatekey",
    "credential",
    "cookie",
    "set-cookie",
    "authorization",
];

/// Query keys, redacted only as a whole `?key=` or `&sig=` parameter, so `monkey=` is left alone.
const QUERY_SECRET_KEYS: [&str; 2] = ["key", "sig"];

/// Prefixes of tokens that are secrets whatever key they sit under: vendor API keys, GitHub tokens, pairing offers.
const TOKEN_PREFIXES: [&str; 8] = [
    "sk-",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    massorca_protocol::pairing::OFFER_PREFIX,
];

/// Token bytes a prefixed secret needs after its prefix, so `sk-1` or `ghp_short` is not taken for one.
const MIN_TOKEN_BODY: usize = 16;

/// The quotes that open a value: an escaped `\"` inside a Debug-formatted string, or a plain one.
const OPENING_QUOTES: [&[u8]; 3] = [b"\\\"", b"\"", b"'"];

/// Formats each event with `inner`, then redacts the line it wrote.
#[derive(Debug)]
pub struct RedactedFormat<F>(pub F);

impl<S, N, F> FormatEvent<S, N> for RedactedFormat<F>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
    F: FormatEvent<S, N>,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut line = String::new();
        self.0.format_event(ctx, Writer::new(&mut line), event)?;
        writer.write_str(&redact_secret_values(&line))
    }
}

/// Replaces the value of each secret-looking `key=value` or `key: value` in `line` with `[redacted]`, and the password of
/// each URL with credentials. A quoted value ends at its closing quote; a bare one at a space, a quote or a separator.
pub fn redact_secret_values(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut redacted = String::with_capacity(line.len());
    let (mut copied, mut at) = (0, 0);
    while at < bytes.len() {
        match secret_value_at(bytes, at) {
            Some((start, end)) => {
                redacted.push_str(&line[copied..start]);
                redacted.push_str(REDACTED);
                (copied, at) = (end, end);
            }
            None => at += 1,
        }
    }
    redacted.push_str(&line[copied..]);
    redacted
}

/// The byte span redacted at `at`: a URL password whose `://` begins there, or the value of a secret key.
fn secret_value_at(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
    url_password_at(bytes, at)
        .or_else(|| prefixed_token_at(bytes, at))
        .or_else(|| key_value_at(bytes, at))
}

/// The span of a token with a known secret prefix that begins at `at`, as a whole word.
fn prefixed_token_at(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
    if at > 0 && is_word_byte(bytes[at - 1]) {
        return None;
    }
    let prefix = TOKEN_PREFIXES
        .iter()
        .find(|prefix| bytes[at..].starts_with(prefix.as_bytes()))?;
    let body = at + prefix.len();
    let end = skip_while(bytes, body, is_token_byte);
    (end - body >= MIN_TOKEN_BODY).then_some((at, end))
}

/// The byte span of the value of the secret key or query key that begins at `at`, if it has a value.
fn key_value_at(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
    if let Some(key_len) = query_key_len(bytes, at) {
        return value_span(bytes, at + key_len + 1, false);
    }
    let key = SECRET_KEYS
        .iter()
        .find(|key| starts_with_ignore_case(&bytes[at..], key))?;
    let mut i = skip_while(bytes, at + key.len(), is_word_byte);
    i = skip_quote(bytes, i);
    i = skip_while(bytes, i, |byte| byte == b' ');
    if !matches!(bytes.get(i), Some(b'=' | b':')) {
        return None;
    }
    let value = skip_while(bytes, i + 1, |byte| byte == b' ');
    value_span(bytes, value, *key == "authorization")
}

/// The length of the query key at `at` when it is a whole `?key=` or `&sig=` parameter name.
fn query_key_len(bytes: &[u8], at: usize) -> Option<usize> {
    let after_separator = at > 0 && matches!(bytes[at - 1], b'?' | b'&');
    QUERY_SECRET_KEYS.iter().find_map(|key| {
        let whole = after_separator
            && starts_with_ignore_case(&bytes[at..], key)
            && bytes.get(at + key.len()) == Some(&b'=');
        whole.then_some(key.len())
    })
}

/// The byte span of a value that begins at `from`: between its quotes when it opens with one, else up to a value end.
fn value_span(bytes: &[u8], from: usize, is_authorization: bool) -> Option<(usize, usize)> {
    let Some(quote) = opening_quote(bytes, from) else {
        return unquoted_value_span(bytes, from, is_authorization);
    };
    let start = from + quote.len();
    let end = closing_quote(bytes, start, quote);
    (end > start).then_some((start, end))
}

/// A bare value. After `authorization` or a `Bearer` word, the credential that follows the scheme is redacted too.
fn unquoted_value_span(
    bytes: &[u8],
    start: usize,
    is_authorization: bool,
) -> Option<(usize, usize)> {
    let end = unquoted_end(bytes, start);
    if end == start {
        return None;
    }
    if is_authorization || bytes[start..end].eq_ignore_ascii_case(b"bearer") {
        let credential = skip_while(bytes, end, |byte| byte == b' ');
        let credential_end = unquoted_end(bytes, credential);
        if credential_end > credential {
            return Some((start, credential_end));
        }
    }
    Some((start, end))
}

/// The index of the quote that closes a value opened by `quote`, skipping escaped bytes; `bytes.len()` if none does.
fn closing_quote(bytes: &[u8], from: usize, quote: &[u8]) -> usize {
    let mut i = from;
    while i < bytes.len() && !bytes[i..].starts_with(quote) {
        i += if bytes[i] == b'\\' { 2 } else { 1 };
    }
    i.min(bytes.len())
}

/// Where a bare value that begins at `from` ends: at a value end, or at the escaped quote closing a Debug string.
fn unquoted_end(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < bytes.len() && !is_value_end(bytes[i]) && !bytes[i..].starts_with(b"\\\"") {
        i += 1;
    }
    i
}

/// The quote that starts at `at`, if one does.
fn opening_quote(bytes: &[u8], at: usize) -> Option<&'static [u8]> {
    OPENING_QUOTES
        .into_iter()
        .find(|quote| bytes[at..].starts_with(quote))
}

/// Skips the quote at `at`, if one does, escaped or not.
fn skip_quote(bytes: &[u8], at: usize) -> usize {
    opening_quote(bytes, at).map_or(at, |quote| at + quote.len())
}

/// The span of the password in a `scheme://user:password@host` URL whose `://` begins at `at`.
fn url_password_at(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
    if !bytes[at..].starts_with(b"://") {
        return None;
    }
    let authority = at + 3;
    let end = skip_while(bytes, authority, |byte| !is_authority_end(byte));
    let at_sign = (authority..end).rev().find(|&i| bytes[i] == b'@')?;
    let colon = (authority..at_sign).find(|&i| bytes[i] == b':')?;
    (colon + 1 < at_sign).then_some((colon + 1, at_sign))
}

fn starts_with_ignore_case(haystack: &[u8], needle: &str) -> bool {
    haystack.len() >= needle.len()
        && haystack[..needle.len()].eq_ignore_ascii_case(needle.as_bytes())
}

/// The index of the first byte from `from` that `keep` refuses, or the end of `bytes`.
fn skip_while(bytes: &[u8], from: usize, keep: impl Fn(u8) -> bool) -> usize {
    let mut i = from;
    while i < bytes.len() && keep(bytes[i]) {
        i += 1;
    }
    i
}

/// Bytes a key may be followed by before its separator, as in `client_secret` or `api-key`.
fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

/// Bytes of an API key, a GitHub token or a base64url pairing offer.
fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
}

/// Bytes that end a bare value: white space, a quote, or a separator closing a JSON, query or list item.
fn is_value_end(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(byte, b'"' | b'\'' | b',' | b';' | b'&' | b'}' | b']' | b')')
}

/// Bytes that end a URL's authority: white space, the start of a path, query or fragment, or a quote or escape.
fn is_authority_end(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b'/' | b'?' | b'#' | b'"' | b'\'' | b'\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_looking_values_are_redacted_in_key_value_and_json_forms() {
        assert_eq!(
            redact_secret_values("token=abc123 next"),
            "token=[redacted] next"
        );
        assert_eq!(
            redact_secret_values("password: hunter2"),
            "password: [redacted]"
        );
        assert_eq!(
            redact_secret_values(r#"{"api_key": "sk-1", "n": 2}"#),
            r#"{"api_key": "[redacted]", "n": 2}"#
        );
        assert_eq!(
            redact_secret_values("MASSORCA_TOKEN=xyz"),
            "MASSORCA_TOKEN=[redacted]"
        );
        assert_eq!(
            redact_secret_values("client_secret=s3cr3t, next"),
            "client_secret=[redacted], next"
        );
    }

    #[test]
    fn a_bearer_token_is_redacted_with_its_scheme() {
        assert_eq!(
            redact_secret_values("Authorization: Bearer abc.def ok"),
            "Authorization: [redacted] ok"
        );
    }

    #[test]
    fn ordinary_words_counts_and_empty_values_are_left_alone() {
        assert_eq!(redact_secret_values("token count: 5"), "token count: 5");
        assert_eq!(
            redact_secret_values("the tokenizer runs"),
            "the tokenizer runs"
        );
        assert_eq!(redact_secret_values("secret"), "secret");
        assert_eq!(redact_secret_values("token="), "token=");
    }

    #[test]
    fn text_around_a_redacted_value_is_kept_intact() {
        assert_eq!(redact_secret_values("é token=x ü"), "é token=[redacted] ü");
    }

    #[test]
    fn a_secret_inside_an_escaped_json_string_field_is_redacted() {
        assert_eq!(
            redact_secret_values(r#"body="{\"token\":\"abc\"}""#),
            r#"body="{\"token\":\"[redacted]\"}""#
        );
    }

    #[test]
    fn a_quoted_value_with_spaces_is_redacted_up_to_its_closing_quote() {
        assert_eq!(
            redact_secret_values(r#"password="correct horse battery" next"#),
            r#"password="[redacted]" next"#
        );
        assert_eq!(
            redact_secret_values(r#"password="a\"b c" ok"#),
            r#"password="[redacted]" ok"#
        );
        assert_eq!(
            redact_secret_values(r#"body="{\"password\":\"correct horse\"}""#),
            r#"body="{\"password\":\"[redacted]\"}""#
        );
    }

    #[test]
    fn an_authorization_credential_is_redacted_whatever_its_scheme() {
        assert_eq!(
            redact_secret_values("authorization=Basic dXNlcjpwYXNz next"),
            "authorization=[redacted] next"
        );
        assert_eq!(
            redact_secret_values("Authorization: Token abc"),
            "Authorization: [redacted]"
        );
    }

    #[test]
    fn more_secret_keys_query_values_and_url_passwords_are_redacted() {
        assert_eq!(
            redact_secret_values("passphrase=hunter2 next"),
            "passphrase=[redacted] next"
        );
        assert_eq!(
            redact_secret_values("privateKey: abc"),
            "privateKey: [redacted]"
        );
        assert_eq!(
            redact_secret_values("cookie: sid=abc; theme=dark"),
            "cookie: [redacted]; theme=dark"
        );
        assert_eq!(
            redact_secret_values("set-cookie: sid=abc"),
            "set-cookie: [redacted]"
        );
        assert_eq!(redact_secret_values("pwd=/home/me"), "pwd=[redacted]");
        assert_eq!(
            redact_secret_values("GET /api?key=abc&sig=xyz"),
            "GET /api?key=[redacted]&sig=[redacted]"
        );
        assert_eq!(
            redact_secret_values("monkey=banana ?keyboard=qwerty"),
            "monkey=banana ?keyboard=qwerty"
        );
        assert_eq!(
            redact_secret_values("https://user:hunter2@example.com/x"),
            "https://user:[redacted]@example.com/x"
        );
        assert_eq!(
            redact_secret_values("http://example.com:8080/x"),
            "http://example.com:8080/x"
        );
    }

    #[test]
    fn tokens_with_a_known_secret_prefix_are_redacted_under_any_name() {
        assert_eq!(
            redact_secret_values("using sk-ant-api03-AbCdEf0123456789xyz now"),
            "using [redacted] now"
        );
        assert_eq!(
            redact_secret_values("remote ghp_0123456789abcdefABCDEF0123 set"),
            "remote [redacted] set"
        );
        assert_eq!(
            redact_secret_values("offer=\"mop1.eyJ2IjoxLCJob3N0IjoiZXhhbXBsZSJ9\""),
            "offer=\"[redacted]\""
        );
        assert_eq!(
            redact_secret_values("risk-free ghp_short desk-1"),
            "risk-free ghp_short desk-1"
        );
    }
}
