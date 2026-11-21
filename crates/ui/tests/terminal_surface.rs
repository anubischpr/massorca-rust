//! GPUI tests of the terminal surface on the fake platform: keys and pastes reach the
//! owner's callback as the bytes the program reads (docs/design/components/TerminalSurface.md).
//! No window is shown.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{ClipboardItem, Entity, TestAppContext, VisualTestContext};
use massorca_ui::components::terminal_surface::TerminalSurface;
use massorca_ui::components::terminal_surface::feed::FeedEvent;

/// The bytes the surface has sent to its program.
type Sent = Rc<RefCell<Vec<u8>>>;

/// A surface in a test window that records what it sends, focused.
fn focused_surface(
    cx: &mut TestAppContext,
) -> (Entity<TerminalSurface>, Sent, &mut VisualTestContext) {
    let sent = Sent::default();
    let recorder = Rc::clone(&sent);
    let (surface, cx) = cx.add_window_view(|_, cx| {
        TerminalSurface::new(80, 24, cx).on_input(move |bytes, _, _| {
            recorder.borrow_mut().extend_from_slice(bytes);
        })
    });
    let focus = surface.update(cx, |surface, _| surface.focus_handle().clone());
    cx.update(|window, cx| focus.focus(window, cx));
    (surface, sent, cx)
}

/// Feeds the surface a fresh attach followed by `bytes` of program output.
fn feed_output(surface: &Entity<TerminalSurface>, cx: &mut VisualTestContext, bytes: &[u8]) {
    surface.update(cx, |surface, cx| {
        surface
            .apply(FeedEvent::Attached { offset: 0 }, cx)
            .expect("attach");
        surface
            .apply(FeedEvent::Output { offset: 0, bytes }, cx)
            .expect("output");
    });
}

#[gpui::test]
fn typed_keys_reach_the_program_as_bytes(cx: &mut TestAppContext) {
    let (_surface, sent, cx) = focused_surface(cx);
    cx.simulate_keystrokes("h i enter");
    assert_eq!(sent.borrow().as_slice(), b"hi\r");
}

#[gpui::test]
fn control_chords_send_control_codes(cx: &mut TestAppContext) {
    let (_surface, sent, cx) = focused_surface(cx);
    cx.simulate_keystrokes("ctrl-c");
    assert_eq!(sent.borrow().as_slice(), b"\x03");
}

#[gpui::test]
fn cursor_keys_follow_the_programs_cursor_mode(cx: &mut TestAppContext) {
    let (surface, sent, cx) = focused_surface(cx);
    feed_output(&surface, cx, b"\x1b[?1h");
    cx.simulate_keystrokes("up");
    assert_eq!(sent.borrow().as_slice(), b"\x1bOA");
}

#[gpui::test]
fn a_paste_is_bracketed_only_when_the_program_asks_for_it(cx: &mut TestAppContext) {
    let (surface, sent, cx) = focused_surface(cx);
    cx.write_to_clipboard(ClipboardItem::new_string("one\ntwo".to_owned()));

    cx.simulate_keystrokes("ctrl-shift-v");
    assert_eq!(sent.borrow().as_slice(), b"one\rtwo");

    sent.borrow_mut().clear();
    feed_output(&surface, cx, b"\x1b[?2004h");
    cx.simulate_keystrokes("ctrl-shift-v");
    assert_eq!(sent.borrow().as_slice(), b"\x1b[200~one\ntwo\x1b[201~");
}

#[gpui::test]
fn an_exited_terminal_takes_no_input(cx: &mut TestAppContext) {
    let (surface, sent, cx) = focused_surface(cx);
    surface.update(cx, |surface, cx| {
        surface
            .apply(FeedEvent::Attached { offset: 0 }, cx)
            .expect("attach");
        surface
            .apply(FeedEvent::Exited { code: Some(0) }, cx)
            .expect("exit");
    });
    cx.simulate_keystrokes("a enter");
    assert!(sent.borrow().is_empty());
}

#[gpui::test]
fn keys_the_app_keeps_are_not_sent_to_the_program(cx: &mut TestAppContext) {
    let (_surface, sent, cx) = focused_surface(cx);
    cx.simulate_keystrokes("f6 ctrl-tab");
    assert!(sent.borrow().is_empty());
}
