//! Identifier rules. Each rule is a subset of the server's owner-name rule, so an id that passes here also works as
//! an `ext:<id>` owner there.

/// Agent kind ids: lowercase `[a-z0-9-]`, 1 to 32 characters (0050).
pub fn is_kind_id(text: &str) -> bool {
    segment(text, 32, |byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
    })
}

/// Contribution ids and mode names: lowercase letters, digits, `_` and `-`, 1 to 64 characters, not starting with `-`.
pub fn is_contribution_id(text: &str) -> bool {
    segment(text, 64, |byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
    }) && !text.starts_with('-')
}

/// Setting keys: lowercase letters, digits, `.`, `_` and `-`, starting with a letter or a digit.
pub fn is_setting_key(text: &str) -> bool {
    let mut bytes = text.bytes();
    text.len() <= 64
        && bytes
            .next()
            .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

/// Extension ids: `publisher.name` with exactly one dot, each side a lowercase segment of at most 32 characters.
pub fn is_extension_id(text: &str) -> bool {
    match text.split_once('.') {
        Some((publisher, name)) => {
            is_extension_segment(publisher) && is_extension_segment(name) && !name.contains('.')
        }
        None => false,
    }
}

/// The publisher part of a well-formed extension id.
pub fn publisher_of(text: &str) -> Option<&str> {
    if !is_extension_id(text) {
        return None;
    }
    text.split_once('.').map(|(publisher, _)| publisher)
}

/// Handler names: a JavaScript-style identifier of at most 64 characters.
pub fn is_handler(text: &str) -> bool {
    let mut bytes = text.bytes();
    text.len() <= 64
        && bytes
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// A publisher or name segment. It becomes a folder name, so it follows the same Windows rule as a package path
/// (no device names such as `con` or `com1`).
fn is_extension_segment(text: &str) -> bool {
    segment(text, 32, |byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
    }) && !text.starts_with('-')
        && crate::package_path::windows_name_problem(text).is_none()
}

fn segment(text: &str, max: usize, allowed: impl Fn(u8) -> bool) -> bool {
    !text.is_empty() && text.len() <= max && text.bytes().all(allowed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_ids_follow_the_agent_rule() {
        assert!(is_kind_id("claude"));
        assert!(is_kind_id("gentle-shell"));
        assert!(is_kind_id(&"a".repeat(32)));
        assert!(!is_kind_id(&"a".repeat(33)));
        assert!(!is_kind_id(""));
        assert!(!is_kind_id("Claude"));
        assert!(!is_kind_id("claude_code"));
        assert!(!is_kind_id("a.b"));
    }

    #[test]
    fn contribution_ids_allow_underscores_but_not_leading_dashes() {
        assert!(is_contribution_id("add_note"));
        assert!(is_contribution_id("add-note"));
        assert!(!is_contribution_id("-add"));
        assert!(!is_contribution_id("Add"));
        assert!(!is_contribution_id("a b"));
    }

    #[test]
    fn setting_keys_allow_dots() {
        assert!(is_setting_key("workspaces.default_root"));
        assert!(!is_setting_key(".hidden"));
        assert!(!is_setting_key("Columns"));
    }

    #[test]
    fn extension_ids_are_publisher_dot_name() {
        assert!(is_extension_id("massorca.sticky-wall"));
        assert!(is_extension_id("acme.tool_kit"));
        assert!(!is_extension_id("sticky"));
        assert!(!is_extension_id("a.b.c"));
        assert!(!is_extension_id("Acme.tool"));
        assert!(!is_extension_id(".tool"));
        assert!(!is_extension_id("acme."));
        assert_eq!(publisher_of("acme.tool"), Some("acme"));
        assert_eq!(publisher_of("acme"), None);
    }

    #[test]
    fn extension_id_segments_follow_the_windows_name_rule() {
        for id in ["con.kit", "acme.com1", "lpt9.kit", "acme.aux", "acme.nul"] {
            assert!(!is_extension_id(id), "{id}");
        }
        assert!(is_extension_id("acme.console"));
    }

    #[test]
    fn handlers_are_identifiers() {
        assert!(is_handler("addNote"));
        assert!(is_handler("_on1"));
        assert!(!is_handler("1on"));
        assert!(!is_handler("add-note"));
        assert!(!is_handler(""));
    }
}
