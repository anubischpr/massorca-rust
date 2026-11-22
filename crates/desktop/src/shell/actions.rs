//! Shell actions and their key bindings (handoff 2.3).

use gpui::{App, KeyBinding, actions};

actions!(
    shell,
    [
        CloseOverlay,
        CycleRegionForward,
        CycleRegionBackward,
        RailPrevious,
        RailNext,
        RailFirst,
        RailLast,
        ToggleCommandPalette,
        OpenQuickOpen,
        OpenSettings,
        PaletteUp,
        PaletteDown
    ]
);

/// Binds the shell's keys. Widgets bind their own keys in their `bind_keys`.
pub fn bind_keys(cx: &mut App) {
    // `KeyBinding` takes no `secondary-` modifier, so Mod is spelled out per OS (handoff 2.3).
    let (palette_key, quick_open_key, settings_key) = if cfg!(target_os = "macos") {
        ("cmd-k", "cmd-p", "cmd-,")
    } else {
        ("ctrl-k", "ctrl-p", "ctrl-,")
    };
    cx.bind_keys([
        KeyBinding::new("escape", CloseOverlay, None),
        KeyBinding::new("f6", CycleRegionForward, None),
        KeyBinding::new("shift-f6", CycleRegionBackward, None),
        KeyBinding::new(palette_key, ToggleCommandPalette, None),
        KeyBinding::new(quick_open_key, OpenQuickOpen, None),
        KeyBinding::new(settings_key, OpenSettings, None),
        KeyBinding::new("up", RailPrevious, Some("ActivityRail")),
        KeyBinding::new("down", RailNext, Some("ActivityRail")),
        KeyBinding::new("home", RailFirst, Some("ActivityRail")),
        KeyBinding::new("end", RailLast, Some("ActivityRail")),
        KeyBinding::new("up", PaletteUp, Some("CommandPalette")),
        KeyBinding::new("down", PaletteDown, Some("CommandPalette")),
    ]);
}
