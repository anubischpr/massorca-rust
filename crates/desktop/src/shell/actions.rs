//! Shell actions and their key bindings (handoff 2.3).

use gpui::{App, KeyBinding, actions};

actions!(shell, [CloseOverlay]);

/// Binds the shell's keys. Widgets bind their own keys in their `bind_keys`.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CloseOverlay, None)]);
}
