//! Shell actions and their key bindings (handoff 2.3).

use gpui::{App, KeyBinding, actions};

actions!(
    shell,
    [CloseOverlay, RailPrevious, RailNext, RailFirst, RailLast]
);

/// Binds the shell's keys. Widgets bind their own keys in their `bind_keys`.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", CloseOverlay, None),
        KeyBinding::new("up", RailPrevious, Some("ActivityRail")),
        KeyBinding::new("down", RailNext, Some("ActivityRail")),
        KeyBinding::new("home", RailFirst, Some("ActivityRail")),
        KeyBinding::new("end", RailLast, Some("ActivityRail")),
    ]);
}
