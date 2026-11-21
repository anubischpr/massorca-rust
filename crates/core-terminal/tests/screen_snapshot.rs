//! Round trip: a snapshot fed to a fresh emulator must reproduce the screen it came from.
//! The reference is `vt100` driven directly with the same bytes.

use std::num::NonZeroU16;

use massorca_core_terminal::screen::{SCROLLBACK_ROWS, Screen};

const EXIT_ALTERNATE: &[u8] = b"\x1b[?1049l";

fn size(n: u16) -> NonZeroU16 {
    NonZeroU16::new(n).expect("test sizes are non-zero")
}

fn reference(cols: u16, rows: u16, bytes: &[u8]) -> vt100::Parser {
    let mut parser = vt100::Parser::new(size(rows), size(cols), SCROLLBACK_ROWS);
    parser.process(bytes);
    parser
}

fn assert_visible_matches(reference: &vt100::Parser, target: &vt100::Parser) {
    let (a, b) = (reference.screen(), target.screen());
    assert_eq!(b.size(), a.size(), "size");
    assert_eq!(b.contents(), a.contents(), "visible text");
    assert_eq!(
        b.contents_formatted(),
        a.contents_formatted(),
        "visible cells with attributes"
    );
    assert_eq!(b.cursor_position(), a.cursor_position(), "cursor position");
    assert_eq!(b.hide_cursor(), a.hide_cursor(), "cursor visibility");
    assert_eq!(b.cursor_style(), a.cursor_style(), "cursor style");
    assert_eq!(
        b.application_cursor(),
        a.application_cursor(),
        "application cursor"
    );
    assert_eq!(
        b.application_keypad(),
        a.application_keypad(),
        "application keypad"
    );
    assert_eq!(b.bracketed_paste(), a.bracketed_paste(), "bracketed paste");
    assert_eq!(
        b.mouse_protocol_mode(),
        a.mouse_protocol_mode(),
        "mouse mode"
    );
    assert_eq!(
        b.mouse_protocol_encoding(),
        a.mouse_protocol_encoding(),
        "mouse encoding"
    );
    assert_eq!(
        b.alternate_screen(),
        a.alternate_screen(),
        "alternate screen"
    );
    assert_eq!(
        b.attributes_formatted(),
        a.attributes_formatted(),
        "current drawing attributes"
    );
}

/// Compares every scrollback offset (the live screen is offset 0), row by row.
fn assert_scrollback_matches(reference: &mut vt100::Parser, target: &mut vt100::Parser) {
    let (cols, rows) = reference.screen().size();
    let (cols, rows) = (cols.get(), rows.get());
    let len = reference.screen().scrollback_len();
    assert_eq!(target.screen().scrollback_len(), len, "scrollback length");
    for offset in 0..=len {
        reference.screen_mut().set_scrollback(offset);
        target.screen_mut().set_scrollback(offset);
        for row in 0..rows {
            assert_eq!(
                target.screen().row_wrapped(row),
                reference.screen().row_wrapped(row),
                "wrap flag, offset {offset} row {row}"
            );
        }
        let want: Vec<String> = reference.screen().rows(0, cols).collect();
        let got: Vec<String> = target.screen().rows(0, cols).collect();
        assert_eq!(got, want, "rows at scrollback offset {offset}");
    }
    reference.screen_mut().set_scrollback(0);
    target.screen_mut().set_scrollback(0);
}

/// Snapshots `screen` (twice: the output must not depend on earlier snapshots), feeds it to a
/// fresh emulator of the screen's size and checks it against `reference`.
fn round_trip(screen: &mut Screen, reference: &mut vt100::Parser) -> vt100::Parser {
    let (cols, rows) = screen.size();
    let snapshot = screen.snapshot();
    assert_eq!(
        screen.snapshot(),
        snapshot,
        "a second snapshot must be identical"
    );
    let mut target = vt100::Parser::new(size(rows), size(cols), SCROLLBACK_ROWS);
    target.process(&snapshot);
    assert_visible_matches(reference, &target);
    assert_scrollback_matches(reference, &mut target);
    target
}

/// Drives a screen and a reference with `bytes`, then round-trips.
fn check(cols: u16, rows: u16, bytes: &[u8]) -> vt100::Parser {
    let mut screen = Screen::new(cols, rows);
    screen.process(bytes);
    let mut reference = reference(cols, rows, bytes);
    round_trip(&mut screen, &mut reference)
}

fn lines(count: usize, text: impl Fn(usize) -> String) -> Vec<u8> {
    (0..count)
        .map(|i| format!("{}\r\n", text(i)))
        .collect::<String>()
        .into_bytes()
}

#[test]
fn plain_lines_beyond_the_screen_height_keep_their_scrollback() {
    let target = check(80, 24, &lines(50, |i| format!("line {i}")));
    assert!(target.screen().scrollback_len() > 0);
}

#[test]
fn empty_screen_round_trips() {
    check(80, 24, b"");
}

#[test]
fn sgr_colours_and_bold_over_300_lines() {
    let bytes = lines(300, |i| {
        format!(
            "\x1b[3{}m\x1b[1mrow {i}\x1b[0m \x1b[48;5;{}m{}\x1b[0m \x1b[38;2;{};{};{}mrgb\x1b[0m",
            i % 8,
            i % 256,
            "bg",
            i % 256,
            (i * 3) % 256,
            (i * 7) % 256
        )
    });
    let target = check(80, 24, &bytes);
    assert!(target.screen().scrollback_len() > 200);
}

#[test]
fn row_exactly_cols_wide_in_scrollback_adds_no_blank_line() {
    // Exact width without a wrap, then a row that wraps at exactly `cols`.
    let mut bytes = b"0123456789\r\n".to_vec();
    bytes.extend_from_slice(b"ABCDEFGHIJKLMNOPQRST\r\n");
    bytes.extend(lines(10, |i| format!("x{i}")));
    let target = check(10, 5, &bytes);
    assert!(target.screen().scrollback_len() > 0);
}

#[test]
fn wide_characters_and_emoji_survive() {
    let mut bytes = "中文字符\r\n".as_bytes().to_vec();
    bytes.extend_from_slice("emoji 😀 done\r\n".as_bytes());
    bytes.extend(lines(30, |i| format!("中{i}😀")));
    check(40, 10, &bytes);
}

/// A wide character that does not fit at the last column wraps to the next row, leaving the
/// column blank; vt100 does not flag that row as soft-wrapped, so the snapshot matches exactly.
#[test]
fn wide_character_wrap_at_the_edge_keeps_its_text() {
    let mut bytes = b"abcdefghi".to_vec();
    bytes.extend_from_slice("中文".as_bytes());
    bytes.extend_from_slice(b"\r\n");
    bytes.extend(lines(8, |i| format!("n{i}")));
    check(10, 4, &bytes);
}

#[test]
fn alternate_screen_draws_over_main_scrollback() {
    let mut bytes = lines(40, |i| format!("main {i}"));
    bytes.extend_from_slice(
        b"\x1b[?1049h\x1b[2J\x1b[1;1Htop bar\x1b[3;5Halternate \x1b[7mtext\x1b[0m",
    );
    let mut screen = Screen::new(80, 24);
    screen.process(&bytes);
    let mut reference = reference(80, 24, &bytes);
    let mut target = round_trip(&mut screen, &mut reference);
    assert!(target.screen().alternate_screen());

    // Leaving the alternate screen must show the main screen the snapshot carried.
    reference.process(EXIT_ALTERNATE);
    target.process(EXIT_ALTERNATE);
    assert!(!target.screen().alternate_screen());
    assert_visible_matches(&reference, &target);
    assert_scrollback_matches(&mut reference, &mut target);
}

#[test]
fn input_modes_and_cursor_style_are_restored() {
    let bytes = b"\x1b[?1h\x1b=\x1b[?2004h\x1b[?1000h\x1b[?1006h\x1b[?25l\x1b[5 qhello";
    check(80, 24, bytes);
}

#[test]
fn mouse_motion_modes_round_trip() {
    check(80, 24, b"\x1b[?1002h\x1b[?1005hmotion");
    check(80, 24, b"\x1b[?1003h\x1b[?1006hany motion");
}

#[test]
fn cursor_moved_mid_screen_is_restored() {
    check(80, 24, b"\x1b[2J\x1b[10;20HX\x1b[3;4Hab");
}

#[test]
fn resize_then_more_output_round_trips() {
    let first = lines(30, |i| format!("before {i}"));
    let second = lines(20, |i| format!("after resize {i}"));
    let mut screen = Screen::new(80, 24);
    screen.process(&first);
    screen.resize(60, 12);
    assert_eq!(screen.size(), (60, 12));
    screen.process(&second);

    let mut reference = reference(80, 24, &first);
    reference.set_size(size(12), size(60));
    reference.process(&second);
    round_trip(&mut screen, &mut reference);
}

#[test]
fn scrollback_keeps_only_the_newest_rows() {
    let target = check(40, 24, &lines(2000, |i| format!("n {i}")));
    assert_eq!(target.screen().scrollback_len(), SCROLLBACK_ROWS);
}

#[test]
fn size_is_reported_as_cols_then_rows() {
    let screen = Screen::new(100, 30);
    assert_eq!(screen.size(), (100, 30));
}

#[test]
fn title_is_set_by_osc_0_and_osc_2() {
    let mut screen = Screen::new(80, 24);
    assert_eq!(screen.title(), None);
    screen.process(b"\x1b]0;hello\x07");
    assert_eq!(screen.title(), Some("hello"));
    screen.process(b"\x1b]2;world\x1b\\");
    assert_eq!(screen.title(), Some("world"));
}

#[test]
fn title_is_capped_at_1024_characters() {
    let mut screen = Screen::new(80, 24);
    let long = "x".repeat(2000);
    screen.process(format!("\x1b]2;{long}\x07").as_bytes());
    assert_eq!(screen.title().map(|t| t.chars().count()), Some(1024));
}

/// A soft-wrapped line that straddles the scrollback and the visible screen keeps its join.
#[test]
fn soft_wrapped_line_across_the_scrollback_boundary_keeps_its_join() {
    for lead in 0..12 {
        let mut bytes = lines(lead, |i| format!("p{i}"));
        bytes.extend(std::iter::repeat_n(b'w', 45));
        if lead % 2 == 0 {
            bytes.extend_from_slice(b"\r\n");
        }
        bytes.extend(lines(3, |i| format!("tail {i}")));
        check(20, 5, &bytes);
    }
}

/// A blank row that continues a wrap (its predecessor soft-wraps into it) must keep the wrap.
#[test]
fn blank_row_that_continues_a_wrap_is_kept() {
    let mut bytes = b"abcdefghijk\x1b[2K\r\n".to_vec();
    bytes.extend(lines(8, |i| format!("after {i}")));
    check(10, 4, &bytes);
}

/// Background blanks and erase-to-end-of-line keep their colour without printed text.
#[test]
fn coloured_blanks_and_erase_tails_round_trip() {
    let mut bytes = lines(40, |i| format!("\x1b[44m{i}\x1b[K\x1b[0m"));
    bytes.extend_from_slice(b"\x1b[41m   \x1b[0mtext\x1b[42m   \x1b[0m\r\n");
    bytes.extend_from_slice(b"\x1b[42mgreen bar\x1b[K");
    check(30, 6, &bytes);
}

/// Feeds `bytes` to a reference parser, and to a screen whose snapshot a fresh parser replays.
/// `probe` then goes to both, and they must show the same.
fn probe_matches_reference(bytes: &[u8], probe: &[u8]) {
    let mut screen = Screen::new(80, 24);
    screen.process(bytes);
    let mut target = vt100::Parser::new(size(24), size(80), SCROLLBACK_ROWS);
    target.process(&screen.snapshot());
    let mut reference = reference(80, 24, bytes);
    target.process(probe);
    reference.process(probe);
    assert_visible_matches(&reference, &target);
    assert_scrollback_matches(&mut reference, &mut target);
}

fn snapshot_of(bytes: &[u8]) -> Vec<u8> {
    let mut screen = Screen::new(80, 24);
    screen.process(bytes);
    screen.snapshot()
}

/// The snapshot of a screen rebuilt from the snapshot of `bytes`, after `probe` is fed to it.
fn rebuilt_after(bytes: &[u8], probe: &[u8]) -> Vec<u8> {
    let mut rebuilt = Screen::new(80, 24);
    rebuilt.process(&snapshot_of(bytes));
    rebuilt.process(probe);
    rebuilt.snapshot()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn scroll_region_is_restored() {
    // The line feed at the bottom margin scrolls rows 5 to 10 only, so "bottom" moves up a row.
    probe_matches_reference(
        b"\x1b[2J\x1b[1;1Htop\x1b[5;10r\x1b[10;1Hbottom",
        b"\x1b[10;1H\n",
    );
}

#[test]
fn origin_mode_is_restored() {
    // Under origin mode `ESC[1;1H` is the top of the region, row 4 here.
    probe_matches_reference(b"\x1b[4;12r\x1b[?6h", b"\x1b[1;1HX");
}

#[test]
fn saved_cursor_is_restored_for_esc_8() {
    probe_matches_reference(b"\x1b[3;4H\x1b7\x1b[10;20Htext", b"\x1b8X");
}

#[test]
fn saved_cursor_keeps_its_origin_mode() {
    probe_matches_reference(b"\x1b[5;10r\x1b[?6h\x1b[2;3H\x1b7\x1b[?6l", b"\x1b8X");
}

#[test]
fn csi_s_and_csi_u_save_and_restore_the_cursor() {
    let mut screen = Screen::new(80, 24);
    screen.process(b"\x1b[3;4H\x1b[s\x1b[10;20H\x1b[u");
    assert_eq!(screen.cursor_position(), (2, 3));
    let bytes = b"\x1b[3;4H\x1b[s\x1b[10;20H";
    assert_eq!(
        rebuilt_after(bytes, b"\x1b[uX"),
        snapshot_of(&[&bytes[..], b"\x1b[uX"].concat())
    );
}

#[test]
fn autowrap_off_is_replayed() {
    assert!(contains(&snapshot_of(b"\x1b[?7l"), b"\x1b[?7l"));
}

#[test]
fn line_drawing_and_shifts_are_stored_and_restored() {
    assert!(contains(&snapshot_of(b"\x1b(0lqk\x1b(B"), "┌─┐".as_bytes()));
    assert!(contains(&rebuilt_after(b"\x1b(0", b"q"), "─".as_bytes()));
    let shifted = rebuilt_after(b"\x1b)0\x0e", b"q");
    assert!(contains(&shifted, "─".as_bytes()));
    // After shift in, `q` is ASCII again.
    let unshifted = rebuilt_after(b"\x1b)0\x0eq\x0f", b"q");
    assert!(contains(&unshifted, "─q".as_bytes()));
}

#[test]
fn sequences_split_across_process_calls_are_tracked() {
    let mut screen = Screen::new(80, 24);
    for chunk in [&b"\x1b[4;"[..], b"12r\x1b[?", b"6h\x1b", b"(0", b"q"] {
        screen.process(chunk);
    }
    let snapshot = screen.snapshot();
    assert!(contains(&snapshot, b"\x1b[?6h"));
    assert!(contains(&snapshot, "─".as_bytes()));
    assert_eq!(snapshot, snapshot_of(b"\x1b[4;12r\x1b[?6h\x1b(0q"));
}

#[test]
fn full_reset_forgets_the_restored_modes() {
    let mut screen = Screen::new(80, 24);
    screen.process(b"\x1b(0\x0e\x1b[?7l\x1b[?6h\x1b[3;7r\x1b7\x1b[?47h\x1bc");
    assert_eq!(screen.snapshot(), Screen::new(80, 24).snapshot());
}

#[test]
fn main_margins_and_cursor_survive_a_snapshot_taken_in_the_alternate_screen() {
    // Leaving the alternate screen returns to the main cursor saved when it was entered.
    probe_matches_reference(
        b"\x1b[2J\x1b[1;1Hmain\x1b[2;5r\x1b[4;3H\x1b[?1049h\x1b[?6h\x1b[1;3r\x1b[1;1Halt",
        b"\x1b[?1049l\x1b[5;1H\n",
    );
}

/// The cursor after writing a full row waits to wrap; the next character must still wrap.
#[test]
fn pending_wrap_survives_a_snapshot() {
    for origin in [&b""[..], &b"\x1b[?6h"[..]] {
        let mut bytes = b"\x1b[2;4r".to_vec();
        bytes.extend_from_slice(origin);
        bytes.extend_from_slice(&[b'x'; 80]);
        probe_matches_reference(&bytes, b"Y");
    }
}
