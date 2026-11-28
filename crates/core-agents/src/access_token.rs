//! The access token inside a kind's credential file (decision 0176): the only part of the file that leaves this crate.
//!
//! The file is parsed in place: the token comes back borrowed from the zeroed read buffer, so no copy of the file, its
//! refresh token or any other field is made. Only the token is then copied, once, into a zeroed string. A value that
//! is not a plain JSON string (one with escapes would need a copy to unescape) is refused, and every parse error is
//! dropped: an error message may quote what it read.

use std::fmt;

use serde::de::{self, DeserializeSeed, Deserializer, IgnoredAny, MapAccess, Visitor};

/// Where each kind's credential file keeps its access token: two nested object keys.
const TOKEN_PATHS: &[(&str, [&str; 2])] = &[
    // Claude Code's `.credentials.json`: `{ "claudeAiOauth": { "accessToken": ..., "refreshToken": ... } }`.
    ("claude", ["claudeAiOauth", "accessToken"]),
    // Codex's `auth.json`: `{ "tokens": { "access_token": ..., "refresh_token": ... } }`.
    ("codex", ["tokens", "access_token"]),
];

/// The path of `kind`'s access token in its credential file; `None` when this crate knows none.
pub(crate) fn token_path(kind: &str) -> Option<[&'static str; 2]> {
    TOKEN_PATHS
        .iter()
        .find(|(id, _)| *id == kind)
        .map(|(_, path)| *path)
}

/// The non-empty plain string at `path` in the JSON object `text`, borrowed from `text`; `None` when the text is not
/// one JSON object, a key on the path is missing or repeated, or the value is not a plain non-empty string.
pub(crate) fn find_token<'t>(text: &'t str, path: &[&str]) -> Option<&'t str> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let found = Path(path).deserialize(&mut deserializer).ok()??;
    deserializer.end().ok()?;
    (!found.is_empty()).then_some(found)
}

/// Walks one key of the path, or reads the token when none is left.
struct Path<'p>(&'p [&'p str]);

impl<'de> DeserializeSeed<'de> for Path<'_> {
    type Value = Option<&'de str>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        match self.0.split_first() {
            None => deserializer.deserialize_any(Token),
            Some((name, rest)) => deserializer.deserialize_any(Object { name, rest }),
        }
    }
}

/// The error for a string where none is wanted. It is fixed, so it never quotes the string.
fn string_refused<E: de::Error>() -> E {
    E::custom("a string is not accepted here")
}

/// The token itself: a string borrowed from the input.
struct Token;

impl<'de> Visitor<'de> for Token {
    type Value = Option<&'de str>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a plain string")
    }

    fn visit_borrowed_str<E: de::Error>(self, value: &'de str) -> Result<Self::Value, E> {
        Ok(Some(value))
    }

    // A string with escapes arrives unescaped in a scratch buffer: refused, so it is never copied.
    fn visit_str<E: de::Error>(self, _value: &str) -> Result<Self::Value, E> {
        Err(string_refused())
    }

    fn visit_string<E: de::Error>(self, _value: String) -> Result<Self::Value, E> {
        Err(string_refused())
    }
}

/// An object on the path: the value of `name`, walked with `rest`; every other value is skipped unread.
struct Object<'p> {
    name: &'p str,
    rest: &'p [&'p str],
}

impl<'de> Visitor<'de> for Object<'_> {
    type Value = Option<&'de str>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an object")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
        let mut found: Option<Option<&'de str>> = None;
        while let Some(key) = map.next_key::<&str>()? {
            if key == self.name {
                if found.is_some() {
                    return Err(de::Error::custom("a key is repeated"));
                }
                found = Some(map.next_value_seed(Path(self.rest))?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(found.flatten())
    }

    fn visit_borrowed_str<E: de::Error>(self, _value: &'de str) -> Result<Self::Value, E> {
        Err(string_refused())
    }

    fn visit_str<E: de::Error>(self, _value: &str) -> Result<Self::Value, E> {
        Err(string_refused())
    }

    fn visit_string<E: de::Error>(self, _value: String) -> Result<Self::Value, E> {
        Err(string_refused())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAUDE: [&str; 2] = ["claudeAiOauth", "accessToken"];

    #[test]
    fn each_vendor_kind_has_a_token_path_and_others_have_none() {
        assert_eq!(token_path("claude"), Some(CLAUDE));
        assert_eq!(token_path("codex"), Some(["tokens", "access_token"]));
        assert_eq!(token_path("gentle-shell"), None);
    }

    #[test]
    fn only_the_access_token_is_found_and_it_is_borrowed_from_the_file() {
        let text = r#"{"other":{"accessToken":"no"},"claudeAiOauth":{"refreshToken":"fake-refresh","accessToken":"fake-access","expiresAt":1,"scopes":["a\"b"]},"x":[1,{"y":null}]}"#;
        let token = find_token(text, &CLAUDE).unwrap();
        assert_eq!(token, "fake-access");
        let start = text.as_ptr() as usize;
        let at = token.as_ptr() as usize;
        assert!(
            at >= start && at + token.len() <= start + text.len(),
            "the token is a slice of the file, not a copy"
        );
        let codex = r#"{"OPENAI_API_KEY":null,"tokens":{"id_token":"fake-id","access_token":"fake-codex","refresh_token":"fake-refresh","account_id":"fake-account"},"last_refresh":"t"}"#;
        assert_eq!(
            find_token(codex, &["tokens", "access_token"]),
            Some("fake-codex")
        );
    }

    #[test]
    fn a_missing_empty_or_odd_token_is_none() {
        for text in [
            r#"{}"#,
            r#"{"claudeAiOauth":{}}"#,
            r#"{"claudeAiOauth":{"refreshToken":"fake-refresh"}}"#,
            r#"{"claudeAiOauth":{"accessToken":""}}"#,
            r#"{"claudeAiOauth":{"accessToken":null}}"#,
            r#"{"claudeAiOauth":{"accessToken":5}}"#,
            r#"{"claudeAiOauth":{"accessToken":["fake"]}}"#,
            r#"{"claudeAiOauth":{"accessToken":{"v":"fake"}}}"#,
            r#"{"claudeAiOauth":{"accessToken":"fake\\escaped"}}"#,
            r#"{"claudeAiOauth":"fake-access"}"#,
            r#"{"claudeAiOauth":{"accessToken":"a","accessToken":"b"}}"#,
            r#"{"claudeAiOauth":{"accessToken":"a"},"claudeAiOauth":{"accessToken":"b"}}"#,
            r#"{"claudeAiOauth":{"accessToken":"fake-access"}} trailing"#,
            r#"["fake-access"]"#,
            r#""fake-access""#,
            "not json",
            "",
        ] {
            assert_eq!(find_token(text, &CLAUDE), None, "{text}");
        }
    }
}
