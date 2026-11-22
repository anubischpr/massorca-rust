//! Keys and pastes as the bytes a terminal program reads, with xterm's encodings.
//!
//! Pure: the surface turns a GPUI keystroke into a [`KeyPress`] and sends what
//! [`encode_key`] returns. The cursor mode comes from the emulator, and pasted
//! text is wrapped only when the program asked for bracketed paste.

const ESC: u8 = 0x1b;

/// The modifiers a terminal encodes. The platform key (Cmd on macOS) is never
/// encoded: it belongs to the app's own shortcuts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub alt: bool,
    pub control: bool,
}

/// One key press: the key name as GPUI reports it, the text it typed, and its modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyPress<'a> {
    pub key: &'a str,
    pub text: Option<&'a str>,
    pub mods: Mods,
}

/// The bytes for one key press, or `None` when the key sends nothing.
/// `app_cursor` is the program's application cursor mode (DECCKM).
pub fn encode_key(press: KeyPress<'_>, app_cursor: bool) -> Option<Vec<u8>> {
    named_key(press.key, press.mods, app_cursor).or_else(|| character(press))
}

/// The bytes for pasted text. With `bracketed`, the text sits between the
/// paste markers, and a marker inside the text cannot end the paste early.
pub fn encode_paste(text: &str, bracketed: bool) -> Vec<u8> {
    if bracketed {
        let body = text.replace("\x1b[201~", "");
        format!("\x1b[200~{body}\x1b[201~").into_bytes()
    } else {
        text.replace("\r\n", "\n").replace('\n', "\r").into_bytes()
    }
}

/// Keys that are not characters: controls, cursor and editing keys, and function keys.
fn named_key(key: &str, mods: Mods, app_cursor: bool) -> Option<Vec<u8>> {
    let bytes = match key {
        "enter" => with_alt(mods, b"\r"),
        "tab" if mods.shift => b"\x1b[Z".to_vec(),
        "tab" => with_alt(mods, b"\t"),
        "backspace" => with_alt(mods, if mods.control { b"\x08" } else { b"\x7f" }),
        "escape" => with_alt(mods, b"\x1b"),
        "space" => with_alt(mods, if mods.control { b"\x00" } else { b" " }),
        "up" => cursor_key(b'A', mods, app_cursor),
        "down" => cursor_key(b'B', mods, app_cursor),
        "right" => cursor_key(b'C', mods, app_cursor),
        "left" => cursor_key(b'D', mods, app_cursor),
        "home" => cursor_key(b'H', mods, app_cursor),
        "end" => cursor_key(b'F', mods, app_cursor),
        "pageup" => tilde_key(5, mods),
        "pagedown" => tilde_key(6, mods),
        "insert" => tilde_key(2, mods),
        "delete" => tilde_key(3, mods),
        "f1" => ss3_key(b'P', mods),
        "f2" => ss3_key(b'Q', mods),
        "f3" => ss3_key(b'R', mods),
        "f4" => ss3_key(b'S', mods),
        "f5" => tilde_key(15, mods),
        "f6" => tilde_key(17, mods),
        "f7" => tilde_key(18, mods),
        "f8" => tilde_key(19, mods),
        "f9" => tilde_key(20, mods),
        "f10" => tilde_key(21, mods),
        "f11" => tilde_key(23, mods),
        "f12" => tilde_key(24, mods),
        _ => return None,
    };
    Some(bytes)
}

/// A character key: its control code under Control, otherwise the text it typed.
fn character(press: KeyPress<'_>) -> Option<Vec<u8>> {
    let mods = press.mods;
    let key_char = single_char(press.key);
    let text = press.text.filter(|text| !text.is_empty());
    // Windows reports AltGr as Control with Alt, and the character it types as text.
    let altgr = mods.control && mods.alt && text.is_some();
    let control_code = (mods.control && !altgr)
        .then(|| key_char.and_then(control_code))
        .flatten();
    if let Some(code) = control_code {
        return Some(with_alt(mods, &[code]));
    }
    let typed = match (text, key_char) {
        (Some(text), _) => text.as_bytes().to_vec(),
        (None, Some(key)) => key.to_string().into_bytes(),
        (None, None) => return None,
    };
    if altgr {
        Some(typed)
    } else {
        Some(with_alt(mods, &typed))
    }
}

/// xterm's modifier parameter: 1, plus 1 for Shift, 2 for Alt and 4 for Control.
fn modifier_param(mods: Mods) -> u8 {
    1 + u8::from(mods.shift) + 2 * u8::from(mods.alt) + 4 * u8::from(mods.control)
}

/// Puts an Escape before the bytes when Alt is held, as the meta key does.
fn with_alt(mods: Mods, bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 1);
    if mods.alt {
        out.push(ESC);
    }
    out.extend_from_slice(bytes);
    out
}

/// Arrows, Home and End: `ESC O x` in application cursor mode, `ESC [ x` otherwise,
/// and `ESC [ 1 ; m x` once a modifier is held.
fn cursor_key(letter: u8, mods: Mods, app_cursor: bool) -> Vec<u8> {
    match modifier_param(mods) {
        1 if app_cursor => vec![ESC, b'O', letter],
        1 => vec![ESC, b'[', letter],
        param => format!("\x1b[1;{param}{}", char::from(letter)).into_bytes(),
    }
}

/// Insert, Delete, Page Up and Page Down, and F5 to F12: `ESC [ n ~`.
fn tilde_key(code: u8, mods: Mods) -> Vec<u8> {
    match modifier_param(mods) {
        1 => format!("\x1b[{code}~").into_bytes(),
        param => format!("\x1b[{code};{param}~").into_bytes(),
    }
}

/// F1 to F4: `ESC O x`, or `ESC [ 1 ; m x` with a modifier.
fn ss3_key(letter: u8, mods: Mods) -> Vec<u8> {
    match modifier_param(mods) {
        1 => vec![ESC, b'O', letter],
        param => format!("\x1b[1;{param}{}", char::from(letter)).into_bytes(),
    }
}

fn single_char(key: &str) -> Option<char> {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

/// The C0 control that a Control chord with `key` sends, as xterm maps them.
fn control_code(key: char) -> Option<u8> {
    match key.to_ascii_lowercase() {
        letter @ 'a'..='z' => Some(letter as u8 - b'a' + 1),
        '@' | ' ' | '2' => Some(0x00),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '7' | '/' => Some(0x1f),
        '?' | '8' => Some(0x7f),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Mods = Mods {
        shift: false,
        alt: false,
        control: false,
    };
    const SHIFT: Mods = Mods {
        shift: true,
        ..NONE
    };
    const ALT: Mods = Mods { alt: true, ..NONE };
    const CONTROL: Mods = Mods {
        control: true,
        ..NONE
    };
    const CONTROL_ALT: Mods = Mods {
        control: true,
        alt: true,
        ..NONE
    };

    fn key(name: &str, mods: Mods) -> Option<Vec<u8>> {
        encode_key(
            KeyPress {
                key: name,
                text: None,
                mods,
            },
            false,
        )
    }

    fn typed(name: &str, text: &str, mods: Mods) -> Option<Vec<u8>> {
        encode_key(
            KeyPress {
                key: name,
                text: Some(text),
                mods,
            },
            false,
        )
    }

    fn cursor(name: &str, mods: Mods, app_cursor: bool) -> Option<Vec<u8>> {
        encode_key(
            KeyPress {
                key: name,
                text: None,
                mods,
            },
            app_cursor,
        )
    }

    #[test]
    fn typed_text_is_sent_as_typed() {
        assert_eq!(typed("a", "a", NONE), Some(b"a".to_vec()));
        assert_eq!(typed("a", "A", SHIFT), Some(b"A".to_vec()));
        assert_eq!(typed("é", "é", NONE), Some("é".as_bytes().to_vec()));
    }

    #[test]
    fn a_character_key_without_text_sends_its_key() {
        assert_eq!(key("q", NONE), Some(b"q".to_vec()));
    }

    #[test]
    fn enter_tab_backspace_escape_and_space_send_their_controls() {
        assert_eq!(key("enter", NONE), Some(b"\r".to_vec()));
        assert_eq!(key("tab", NONE), Some(b"\t".to_vec()));
        assert_eq!(key("tab", SHIFT), Some(b"\x1b[Z".to_vec()));
        assert_eq!(key("backspace", NONE), Some(b"\x7f".to_vec()));
        assert_eq!(key("backspace", CONTROL), Some(b"\x08".to_vec()));
        assert_eq!(key("escape", NONE), Some(b"\x1b".to_vec()));
        assert_eq!(key("space", NONE), Some(b" ".to_vec()));
        assert_eq!(key("space", CONTROL), Some(b"\x00".to_vec()));
    }

    #[test]
    fn arrows_follow_the_application_cursor_mode() {
        assert_eq!(cursor("up", NONE, false), Some(b"\x1b[A".to_vec()));
        assert_eq!(cursor("up", NONE, true), Some(b"\x1bOA".to_vec()));
        assert_eq!(cursor("down", NONE, true), Some(b"\x1bOB".to_vec()));
        assert_eq!(cursor("right", NONE, false), Some(b"\x1b[C".to_vec()));
        assert_eq!(cursor("left", NONE, true), Some(b"\x1bOD".to_vec()));
    }

    #[test]
    fn modified_arrows_carry_the_modifier_in_every_mode() {
        assert_eq!(cursor("up", CONTROL, false), Some(b"\x1b[1;5A".to_vec()));
        assert_eq!(cursor("left", SHIFT, false), Some(b"\x1b[1;2D".to_vec()));
        assert_eq!(cursor("right", ALT, false), Some(b"\x1b[1;3C".to_vec()));
        assert_eq!(cursor("up", CONTROL, true), Some(b"\x1b[1;5A".to_vec()));
    }

    #[test]
    fn home_end_paging_insert_and_delete() {
        assert_eq!(cursor("home", NONE, false), Some(b"\x1b[H".to_vec()));
        assert_eq!(cursor("home", NONE, true), Some(b"\x1bOH".to_vec()));
        assert_eq!(cursor("end", NONE, false), Some(b"\x1b[F".to_vec()));
        assert_eq!(key("pageup", NONE), Some(b"\x1b[5~".to_vec()));
        assert_eq!(key("pagedown", NONE), Some(b"\x1b[6~".to_vec()));
        assert_eq!(key("insert", NONE), Some(b"\x1b[2~".to_vec()));
        assert_eq!(key("delete", NONE), Some(b"\x1b[3~".to_vec()));
        assert_eq!(key("delete", CONTROL), Some(b"\x1b[3;5~".to_vec()));
    }

    #[test]
    fn function_keys_use_ss3_then_tilde_codes() {
        assert_eq!(key("f1", NONE), Some(b"\x1bOP".to_vec()));
        assert_eq!(key("f4", NONE), Some(b"\x1bOS".to_vec()));
        assert_eq!(key("f5", NONE), Some(b"\x1b[15~".to_vec()));
        assert_eq!(key("f12", NONE), Some(b"\x1b[24~".to_vec()));
        assert_eq!(key("f1", SHIFT), Some(b"\x1b[1;2P".to_vec()));
        assert_eq!(key("f5", CONTROL), Some(b"\x1b[15;5~".to_vec()));
    }

    #[test]
    fn control_letters_send_control_codes() {
        assert_eq!(typed("c", "c", CONTROL), Some(b"\x03".to_vec()));
        assert_eq!(key("a", CONTROL), Some(b"\x01".to_vec()));
        assert_eq!(key("z", CONTROL), Some(b"\x1a".to_vec()));
        assert_eq!(key("C", CONTROL), Some(b"\x03".to_vec()));
        assert_eq!(key("[", CONTROL), Some(b"\x1b".to_vec()));
        assert_eq!(key("\\", CONTROL), Some(b"\x1c".to_vec()));
    }

    #[test]
    fn alt_puts_an_escape_before_the_key() {
        assert_eq!(typed("x", "x", ALT), Some(b"\x1bx".to_vec()));
        assert_eq!(key("enter", ALT), Some(b"\x1b\r".to_vec()));
        assert_eq!(key("backspace", ALT), Some(b"\x1b\x7f".to_vec()));
    }

    #[test]
    fn altgr_text_with_control_and_alt_is_typed_without_an_escape() {
        assert_eq!(typed("q", "@", CONTROL_ALT), Some(b"@".to_vec()));
        assert_eq!(typed("q", "@", NONE), Some(b"@".to_vec()));
    }

    #[test]
    fn keys_that_are_not_input_send_nothing() {
        assert_eq!(key("shift", SHIFT), None);
        assert_eq!(key("control", CONTROL), None);
        assert_eq!(key("function", NONE), None);
        assert_eq!(key("f13", NONE), None);
    }

    #[test]
    fn a_paste_without_bracketed_mode_is_sent_as_typed_newlines() {
        assert_eq!(encode_paste("ls\n", false), b"ls\r".to_vec());
        assert_eq!(encode_paste("a\r\nb", false), b"a\rb".to_vec());
    }

    #[test]
    fn a_bracketed_paste_is_wrapped_and_cannot_end_early() {
        assert_eq!(encode_paste("ab", true), b"\x1b[200~ab\x1b[201~".to_vec());
        assert_eq!(
            encode_paste("a\x1b[201~b", true),
            b"\x1b[200~ab\x1b[201~".to_vec()
        );
    }
}
