//! Shared editing for text inputs. [`input`] is the platform contract that the single-line
//! field and the multi-line area both implement (decision 0021); [`layout`] and [`text_area`]
//! add the wrapped multi-line surface.

pub(crate) mod input;
pub mod layout;
pub mod text_area;

pub use super::text_field::TextBuffer;

/// Line breaks as a multi-line surface stores them: `\n` only.
pub fn normalize_line_breaks(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_breaks_normalise_to_newlines() {
        assert_eq!(normalize_line_breaks("a\r\nb\rc\nd"), "a\nb\nc\nd");
    }

    #[test]
    fn the_buffer_is_the_single_line_model() {
        let mut buffer = TextBuffer::new("a");
        assert!(buffer.insert("\n"));
        assert_eq!(buffer.text(), "a\n");
        assert_eq!(buffer.cursor(), 2);
    }
}
