//! A snapshot must rebuild the screen in alacritty, the emulator the desktop client draws with, as
//! it rebuilds it in vt100. vt100 is the service's model, so it is the reference.

mod emulator_support;

use emulator_support::{Client, assert_client_matches, assert_model_matches, vt100_model};
use massorca_core_terminal::screen::Screen;

const COLS: u16 = 40;
const ROWS: u16 = 8;

/// Drives a screen with `bytes`, rebuilds it from its snapshot, then types `typed` into the
/// reference, the vt100 rebuild and the client. Returns the client after `typed`.
fn rebuild_then_type(bytes: &[u8], typed: &[u8]) -> Client {
    let mut screen = Screen::new(COLS, ROWS);
    screen.process(bytes);
    let snapshot = screen.snapshot();
    let mut reference = vt100_model(COLS, ROWS, bytes);
    let mut model = vt100_model(COLS, ROWS, &snapshot);
    let mut client = Client::new(COLS, ROWS);
    client.advance(&snapshot);
    assert_model_matches("the snapshot rebuilt by vt100", &reference, &model);
    assert_client_matches("alacritty after the snapshot", &reference, &client);
    reference.process(typed);
    model.process(typed);
    client.advance(typed);
    assert_model_matches("the snapshot rebuilt by vt100, typed", &reference, &model);
    assert_client_matches("alacritty, typed", &reference, &client);
    client
}

#[test]
fn typed_text_after_a_prompt_lands_after_the_prompt() {
    let client = rebuild_then_type(b"$ echo hello\r\nhello\r\n$ ", b"echo third\r\nthird\r\n$ ");
    assert_eq!(client.row_of("third"), Some(3));
}

#[test]
fn a_cursor_at_home_after_the_snapshot_is_drawn_as_before() {
    rebuild_then_type(b"hello\x1b[H", b"third\r\n");
}

#[test]
fn an_alternate_screen_left_behind_does_not_draw_over_the_main_rows() {
    rebuild_then_type(
        b"\x1b[?1049h\x1b[2J\x1b[1;1Hvim text\x1b[?1049lmain 1\r\nmain 2\r\n$ ",
        b"echo third\r\nthird\r\n$ ",
    );
}

#[test]
fn a_live_alternate_screen_is_the_one_the_client_shows() {
    rebuild_then_type(
        b"main 1\r\n$ \x1b[?1049h\x1b[2J\x1b[1;1Htop bar\x1b[3;5Halt text",
        b"\x1b[?1049l",
    );
}

#[test]
fn the_main_grid_restores_its_own_saved_cursor_after_the_alternate_screen() {
    // Main saves (1,3) and the alternate grid saves (3,6); main is live at (5,1) afterwards.
    rebuild_then_type(
        b"abc\r\ndef\x1b7\x1b[?1049h\x1b[4;7H\x1b7\x1b[?1049l\x1b[6;2H",
        b"\x1b8Q",
    );
}

#[test]
fn the_alternate_grid_restores_its_own_saved_cursor_while_live() {
    // Main saves (1,3); the alternate grid saves (3,6) and is live at home.
    rebuild_then_type(
        b"abc\r\ndef\x1b7\x1b[?1049h\x1b[4;7H\x1b7\x1b[1;1H",
        b"\x1b8Q",
    );
}
