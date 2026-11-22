//! Key presses and mouse clicks for the screenshot tests. They reach the window the way the
//! platform does, through `Window::dispatch_event` and `dispatch_keystroke`, and the window is
//! drawn after each one so the next input sees the state it changed.

use gpui::{
    AnyWindowHandle, HeadlessAppContext, Keystroke, Modifiers, MouseButton, MouseDownEvent,
    MouseUpEvent, PlatformInput, point, px,
};

/// Presses each key of `keys` in turn, named as GPUI names them: `"ctrl-k enter"`, `"escape"`.
pub fn press(
    cx: &mut HeadlessAppContext,
    window: AnyWindowHandle,
    keys: &'static str,
) -> anyhow::Result<()> {
    for name in keys.split_whitespace() {
        let keystroke = Keystroke::parse(name)?;
        dispatch(cx, window, keystroke)?;
    }
    Ok(())
}

/// Types `text` one character at a time, as the keyboard would.
pub fn type_text(
    cx: &mut HeadlessAppContext,
    window: AnyWindowHandle,
    text: &str,
) -> anyhow::Result<()> {
    for character in text.chars() {
        let key = character.to_string();
        let keystroke = Keystroke {
            modifiers: Modifiers::default(),
            key: key.clone(),
            key_char: Some(key),
        };
        dispatch(cx, window, keystroke)?;
    }
    Ok(())
}

/// A primary click at `at`, in window pixels.
pub fn click(
    cx: &mut HeadlessAppContext,
    window: AnyWindowHandle,
    at: (f32, f32),
) -> anyhow::Result<()> {
    press_button(cx, window, MouseButton::Left, at)
}

/// A secondary click at `at`, which opens the row menus and the Status bar menu.
pub fn right_click(
    cx: &mut HeadlessAppContext,
    window: AnyWindowHandle,
    at: (f32, f32),
) -> anyhow::Result<()> {
    press_button(cx, window, MouseButton::Right, at)
}

fn press_button(
    cx: &mut HeadlessAppContext,
    window: AnyWindowHandle,
    button: MouseButton,
    at: (f32, f32),
) -> anyhow::Result<()> {
    let position = point(px(at.0), px(at.1));
    let modifiers = Modifiers::default();
    cx.update_window(window, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button,
                position,
                modifiers,
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button,
                position,
                modifiers,
                click_count: 1,
            }),
            cx,
        );
    })?;
    settle(cx, window)
}

fn dispatch(
    cx: &mut HeadlessAppContext,
    window: AnyWindowHandle,
    keystroke: Keystroke,
) -> anyhow::Result<()> {
    cx.update_window(window, |_, window, cx| {
        window.dispatch_keystroke(keystroke, cx);
    })?;
    settle(cx, window)
}

/// Runs the pending work and draws again, so the next input meets the window as it now is.
fn settle(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> anyhow::Result<()> {
    cx.run_until_parked();
    crate::support::draw(cx, window)
}
