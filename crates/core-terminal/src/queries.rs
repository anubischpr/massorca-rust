//! The terminal queries a program waits on, found across reads, and the colours the service
//! answers with when no emulator is attached (decisions 0034, 0036).

use std::sync::atomic::{AtomicBool, Ordering};

/// The palette the service reports to a program's colour queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TerminalTheme {
    #[default]
    Dark,
    Light,
}

/// Default foreground, background and cursor, as `(red, green, blue)`.
struct DefaultColours {
    foreground: (u8, u8, u8),
    background: (u8, u8, u8),
    cursor: (u8, u8, u8),
}

// Copied from the desktop's terminal tokens (crates/ui/src/tokens/generated.rs, `Palette::{DARK,LIGHT}.terminal`).
const DARK: DefaultColours = DefaultColours {
    foreground: (220, 220, 224),
    background: (16, 16, 18),
    cursor: (232, 232, 236),
};
const LIGHT: DefaultColours = DefaultColours {
    foreground: (31, 31, 38),
    background: (248, 248, 250),
    cursor: (31, 31, 38),
};

/// The theme every terminal of a manager answers with; it may change while they run.
#[derive(Debug, Default)]
pub(crate) struct ThemeCell(AtomicBool);

impl ThemeCell {
    pub(crate) fn get(&self) -> TerminalTheme {
        if self.0.load(Ordering::Relaxed) {
            TerminalTheme::Light
        } else {
            TerminalTheme::Dark
        }
    }

    pub(crate) fn set(&self, theme: TerminalTheme) {
        self.0
            .store(theme == TerminalTheme::Light, Ordering::Relaxed);
    }
}

/// A query found in a program's output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Query {
    /// `ESC [ 6 n`: the cursor position.
    Cursor,
    /// `ESC ] 10|11|12 ; ?` ended by BEL or by ST (`ESC \`): foreground, background or cursor colour.
    Colour { slot: u8, st: bool },
}

const QUERIES: [(&[u8], Query); 7] = [
    (b"\x1b[6n", Query::Cursor),
    (
        b"\x1b]10;?\x07",
        Query::Colour {
            slot: 10,
            st: false,
        },
    ),
    (
        b"\x1b]11;?\x07",
        Query::Colour {
            slot: 11,
            st: false,
        },
    ),
    (
        b"\x1b]12;?\x07",
        Query::Colour {
            slot: 12,
            st: false,
        },
    ),
    (b"\x1b]10;?\x1b\\", Query::Colour { slot: 10, st: true }),
    (b"\x1b]11;?\x1b\\", Query::Colour { slot: 11, st: true }),
    (b"\x1b]12;?\x1b\\", Query::Colour { slot: 12, st: true }),
];
const LONGEST: usize = 8;

/// Finds queries byte by byte, so one split across two reads is still found.
#[derive(Debug, Default)]
pub(crate) struct QueryScanner {
    matched: [u8; LONGEST],
    len: usize,
}

impl QueryScanner {
    /// Takes the next output byte; returns the query it completes, if any.
    pub(crate) fn feed(&mut self, byte: u8) -> Option<Query> {
        if self.len == LONGEST {
            self.drop_first();
        }
        self.matched[self.len] = byte;
        self.len += 1;
        loop {
            let matched = &self.matched[..self.len];
            if let Some((_, query)) = QUERIES.iter().find(|(pattern, _)| *pattern == matched) {
                self.len = 0;
                return Some(*query);
            }
            if self.len == 0
                || QUERIES
                    .iter()
                    .any(|(pattern, _)| pattern.starts_with(matched))
            {
                return None;
            }
            // Not a prefix of any query: retry from the next byte, which may start one.
            self.drop_first();
        }
    }

    fn drop_first(&mut self) {
        self.matched.copy_within(1..self.len, 0);
        self.len -= 1;
    }
}

/// The xterm reply to a colour query: `ESC ] slot ; rgb:RRRR/GGGG/BBBB`, ended as the query was.
pub(crate) fn colour_reply(slot: u8, st: bool, theme: TerminalTheme) -> Vec<u8> {
    let colours = match theme {
        TerminalTheme::Dark => &DARK,
        TerminalTheme::Light => &LIGHT,
    };
    let (red, green, blue) = match slot {
        10 => colours.foreground,
        11 => colours.background,
        _ => colours.cursor,
    };
    let wide = |channel: u8| u16::from(channel) * 257;
    let end = if st { "\x1b\\" } else { "\x07" };
    format!(
        "\x1b]{slot};rgb:{:04x}/{:04x}/{:04x}{end}",
        wide(red),
        wide(green),
        wide(blue)
    )
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::{Query, QueryScanner, TerminalTheme, colour_reply};

    fn scan(scanner: &mut QueryScanner, bytes: &[u8]) -> Vec<Query> {
        bytes
            .iter()
            .filter_map(|byte| scanner.feed(*byte))
            .collect()
    }

    #[test]
    fn finds_each_query_form() {
        let mut scanner = QueryScanner::default();
        assert_eq!(scan(&mut scanner, b"a\x1b[6nb"), [Query::Cursor]);
        assert_eq!(
            scan(&mut scanner, b"\x1b]11;?\x07\x1b]10;?\x1b\\x\x1b]12;?\x07"),
            [
                Query::Colour {
                    slot: 11,
                    st: false
                },
                Query::Colour { slot: 10, st: true },
                Query::Colour {
                    slot: 12,
                    st: false
                },
            ]
        );
    }

    #[test]
    fn finds_a_query_split_across_reads() {
        let mut scanner = QueryScanner::default();
        assert!(scan(&mut scanner, b"out\x1b]11").is_empty());
        assert!(scan(&mut scanner, b";?\x1b").is_empty());
        assert_eq!(
            scan(&mut scanner, b"\\more"),
            [Query::Colour { slot: 11, st: true }]
        );
    }

    #[test]
    fn a_broken_query_does_not_hide_the_next_one() {
        let mut scanner = QueryScanner::default();
        // An unfinished colour query whose terminator turns out to start a cursor query.
        assert_eq!(scan(&mut scanner, b"\x1b]11;?\x1b[6n"), [Query::Cursor]);
        // A colour that is set, not asked, and an OSC slot nobody answers.
        assert!(scan(&mut scanner, b"\x1b]11;#000000\x07\x1b]13;?\x07").is_empty());
    }

    #[test]
    fn replies_in_the_xterm_form_with_the_query_terminator() {
        assert_eq!(
            colour_reply(11, false, TerminalTheme::Dark),
            b"\x1b]11;rgb:1010/1010/1212\x07"
        );
        assert_eq!(
            colour_reply(10, true, TerminalTheme::Light),
            b"\x1b]10;rgb:1f1f/1f1f/2626\x1b\\"
        );
        assert_eq!(
            colour_reply(12, false, TerminalTheme::Dark),
            b"\x1b]12;rgb:e8e8/e8e8/ecec\x07"
        );
    }
}
