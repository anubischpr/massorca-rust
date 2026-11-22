//! Characters a manifest may not carry into the review: control characters, and Unicode format characters that hide
//! or reorder what the person reads (bidirectional overrides and isolates, zero-width spaces, the byte order mark).

/// The first control or format character in `text`, if any.
pub fn hidden_char(text: &str) -> Option<char> {
    text.chars().find(|&c| c.is_control() || is_format(c))
}

/// Unicode format characters a review must never show or run.
pub fn is_format(c: char) -> bool {
    matches!(
        c,
        '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_format_and_control_characters_and_keeps_ordinary_text() {
        for hidden in [
            '\u{202E}', '\u{2066}', '\u{200B}', '\u{200F}', '\u{FEFF}', '\u{7}',
        ] {
            assert_eq!(hidden_char(&format!("a{hidden}b")), Some(hidden));
        }
        assert_eq!(hidden_char("Café — 日本語 ✓"), None);
    }
}
