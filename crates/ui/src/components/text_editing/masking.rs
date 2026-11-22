//! The bullets a masked field paints in place of its text. Each character paints one bullet, so the
//! caret, selection and pointer hits can use one mapping between the text and the painted line.

/// The glyph a masked field paints for each character: U+2022, as the vault artboards draw it.
pub(crate) const MASK_GLYPH: char = '\u{2022}';

/// The text a masked field paints for `text`: one bullet per character.
pub(crate) fn masked_text(text: &str) -> String {
    MASK_GLYPH.to_string().repeat(text.chars().count())
}

/// The byte offset in the painted text of the character that starts at `offset` in `text`.
pub(crate) fn masked_offset(text: &str, offset: usize) -> usize {
    let characters = text
        .char_indices()
        .take_while(|&(index, _)| index < offset)
        .count();
    characters * MASK_GLYPH.len_utf8()
}

/// The byte offset in `text` of the character painted by the bullet at `painted` in the painted text.
pub(crate) fn text_offset(text: &str, painted: usize) -> usize {
    text.char_indices()
        .nth(painted / MASK_GLYPH.len_utf8())
        .map_or(text.len(), |(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_character_paints_one_bullet() {
        assert_eq!(masked_text(""), "");
        assert_eq!(masked_text("a漢🙂"), "•••");
        assert_eq!(
            masked_text("e\u{301}"),
            "••",
            "a combining mark is a character of its own"
        );
    }

    #[test]
    fn offsets_map_characters_to_bullets_across_multibyte_text() {
        // "e" is 1 byte, "🙂" is 4 bytes and "漢" is 3 bytes; a bullet is 3 bytes.
        let text = "e🙂漢";
        assert_eq!(masked_offset(text, 0), 0);
        assert_eq!(masked_offset(text, 1), 3);
        assert_eq!(masked_offset(text, 5), 6);
        assert_eq!(masked_offset(text, text.len()), 9);
    }

    #[test]
    fn offsets_map_bullets_back_to_characters() {
        let text = "e🙂漢";
        assert_eq!(text_offset(text, 0), 0);
        assert_eq!(text_offset(text, 3), 1);
        assert_eq!(text_offset(text, 6), 5);
        assert_eq!(text_offset(text, 9), text.len());
        assert_eq!(
            text_offset(text, 30),
            text.len(),
            "past the last bullet is the end"
        );
    }

    #[test]
    fn every_character_boundary_round_trips() {
        let text = "a漢🙂b";
        for (offset, _) in text.char_indices().chain([(text.len(), '\0')]) {
            assert_eq!(text_offset(text, masked_offset(text, offset)), offset);
        }
    }
}
