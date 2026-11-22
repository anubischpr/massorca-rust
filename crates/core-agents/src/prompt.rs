//! Typed prompts (decision 0054): the bytes a prompt types into a run's terminal, and whether a run takes one now.
//!
//! Pure: the service writes the bytes to the terminal, and reads a run's activity under its status lock.

use massorca_protocol::methods::agents::ActivityState;

use crate::kind::PasteMode;

/// Starts a bracketed paste (xterm mode 2004).
pub const PASTE_START: &str = "\x1b[200~";
/// Ends a bracketed paste. The text never holds it: `launch::check_prompt` refuses the escape character (decision 0058).
pub const PASTE_END: &str = "\x1b[201~";

/// The bytes one prompt types: the text, then the kind's submit sequence. The text must have passed
/// `launch::check_prompt`, which refuses the escape character, so bracketed text cannot end its own paste.
///
/// Bracketed text sits between the paste markers and keeps its line breaks, so a multi-line prompt reaches the agent as
/// one paste. Plain text is typed as keystrokes, each line break as an Enter (`\r`), the same as the desktop's paste.
pub fn input_bytes(text: &str, paste: PasteMode, submit: &str) -> Vec<u8> {
    let mut bytes = match paste {
        PasteMode::Bracketed => format!("{PASTE_START}{text}{PASTE_END}").into_bytes(),
        PasteMode::Plain => text.replace("\r\n", "\n").replace('\n', "\r").into_bytes(),
    };
    bytes.extend_from_slice(submit.as_bytes());
    bytes
}

/// Whether a run takes a prompt now. A kind without a status mechanism always does, since its readiness is never
/// reported. Any other kind takes one when its activity is `idle` or `done`: absent (nothing reported yet) and
/// `working`, `waiting` or an unknown state are not ready.
pub fn ready(has_status: bool, state: Option<ActivityState>) -> bool {
    !has_status || matches!(state, Some(ActivityState::Idle | ActivityState::Done))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_typed_with_the_submit_sequence_last() {
        assert_eq!(input_bytes("hold", PasteMode::Plain, "\r"), b"hold\r");
        assert_eq!(input_bytes("go", PasteMode::Plain, "\n"), b"go\n");
        assert_eq!(input_bytes("", PasteMode::Plain, "\r"), b"\r");
    }

    #[test]
    fn plain_line_breaks_are_typed_as_enter() {
        assert_eq!(input_bytes("a\nb", PasteMode::Plain, "\r"), b"a\rb\r");
        assert_eq!(input_bytes("a\r\nb", PasteMode::Plain, "\r"), b"a\rb\r");
    }

    #[test]
    fn bracketed_text_sits_between_the_markers_and_keeps_its_line_breaks() {
        assert_eq!(
            input_bytes("a\nb", PasteMode::Bracketed, "\r"),
            b"\x1b[200~a\nb\x1b[201~\r"
        );
        assert_eq!(
            input_bytes("hi", PasteMode::Bracketed, "\n"),
            b"\x1b[200~hi\x1b[201~\n"
        );
    }

    #[test]
    fn readiness_follows_the_activity_unless_the_kind_has_no_status() {
        assert!(ready(false, None), "no status: always ready");
        assert!(ready(false, Some(ActivityState::Working)));
        assert!(ready(true, Some(ActivityState::Idle)));
        assert!(ready(true, Some(ActivityState::Done)));
        assert!(!ready(true, None), "absent is not ready");
        assert!(!ready(true, Some(ActivityState::Working)));
        assert!(!ready(true, Some(ActivityState::Waiting)));
        assert!(!ready(true, Some(ActivityState::Unknown)));
    }
}
