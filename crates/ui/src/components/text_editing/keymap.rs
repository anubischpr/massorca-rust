//! The editing keys the single-line field and the multi-line area share, per platform: Command on
//! macOS, Ctrl elsewhere. Each input binds these keys to its own actions.

/// What a shared editing key does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditShortcut {
    Undo,
    Redo,
}

const MACOS: &[(&str, EditShortcut)] = &[
    ("cmd-z", EditShortcut::Undo),
    ("cmd-shift-z", EditShortcut::Redo),
];

const OTHER_PLATFORMS: &[(&str, EditShortcut)] = &[
    ("ctrl-z", EditShortcut::Undo),
    ("ctrl-y", EditShortcut::Redo),
    ("ctrl-shift-z", EditShortcut::Redo),
];

/// The shared editing keys on one platform; `mac` picks the macOS table.
pub(crate) fn shortcuts(mac: bool) -> &'static [(&'static str, EditShortcut)] {
    if mac { MACOS } else { OTHER_PLATFORMS }
}

/// Undo and redo as the platform spells them, for the input tests.
#[cfg(test)]
pub(crate) const UNDO_KEYS: &str = if cfg!(target_os = "macos") {
    "cmd-z"
} else {
    "ctrl-z"
};

#[cfg(test)]
pub(crate) const REDO_KEYS: &str = if cfg!(target_os = "macos") {
    "cmd-shift-z"
} else {
    "ctrl-y"
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_and_redo_use_command_on_macos_and_control_elsewhere() {
        assert!(shortcuts(true).contains(&("cmd-z", EditShortcut::Undo)));
        assert!(shortcuts(true).contains(&("cmd-shift-z", EditShortcut::Redo)));
        assert!(shortcuts(false).contains(&("ctrl-z", EditShortcut::Undo)));
        assert!(shortcuts(false).contains(&("ctrl-y", EditShortcut::Redo)));
        assert!(shortcuts(false).contains(&("ctrl-shift-z", EditShortcut::Redo)));
        assert!(
            shortcuts(true)
                .iter()
                .all(|(keys, _)| !keys.starts_with("ctrl-"))
        );
        assert!(
            shortcuts(false)
                .iter()
                .all(|(keys, _)| !keys.starts_with("cmd-"))
        );
    }
}
