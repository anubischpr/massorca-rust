//! The names owner ids and setting keys may use: lower-case letters, digits, '.', '_' and '-', starting with a letter or a
//! digit. The unit separator the settings registry joins an owner and a key with is outside the set, so the join is
//! unambiguous.

/// Whether `name` follows the rule.
pub(crate) fn is_valid(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::is_valid;

    #[test]
    fn accepts_names_in_the_rule_and_refuses_the_rest() {
        assert!(is_valid("demo"));
        assert!(is_valid("workspaces.default_root"));
        assert!(is_valid("9-lives"));
        assert!(!is_valid(""));
        assert!(!is_valid(".hidden"));
        assert!(!is_valid("Demo"));
        assert!(!is_valid("a b"));
        assert!(!is_valid("a\u{1f}b"));
        assert!(!is_valid("ünï"));
    }
}
