//! The editing keys the single-line field and the multi-line area share, per platform: Command on
//! macOS, Ctrl elsewhere. Each input binds these keys to its own actions.

/// What a shared editing key does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditShortcut {
    Undo,
    Redo,
    /// Word motion and deletion: Option on macOS, Ctrl elsewhere.
    WordLeft,
    WordRight,
    SelectWordLeft,
    SelectWordRight,
    DeleteWordBackward,
    DeleteWordForward,
    /// The caret's line edges: Command+Left and Command+Right on macOS. Home and End are bound by each input.
    LineStart,
    LineEnd,
    SelectLineStart,
    SelectLineEnd,
    /// The whole text's edges: Command+Up and Command+Down on macOS, Ctrl+Home and Ctrl+End elsewhere.
    DocumentStart,
    DocumentEnd,
    SelectDocumentStart,
    SelectDocumentEnd,
}

const MACOS: &[(&str, EditShortcut)] = &[
    ("cmd-z", EditShortcut::Undo),
    ("cmd-shift-z", EditShortcut::Redo),
    ("alt-left", EditShortcut::WordLeft),
    ("alt-right", EditShortcut::WordRight),
    ("alt-shift-left", EditShortcut::SelectWordLeft),
    ("alt-shift-right", EditShortcut::SelectWordRight),
    ("alt-backspace", EditShortcut::DeleteWordBackward),
    ("alt-delete", EditShortcut::DeleteWordForward),
    ("cmd-left", EditShortcut::LineStart),
    ("cmd-right", EditShortcut::LineEnd),
    ("cmd-shift-left", EditShortcut::SelectLineStart),
    ("cmd-shift-right", EditShortcut::SelectLineEnd),
    ("cmd-up", EditShortcut::DocumentStart),
    ("cmd-down", EditShortcut::DocumentEnd),
    ("cmd-shift-up", EditShortcut::SelectDocumentStart),
    ("cmd-shift-down", EditShortcut::SelectDocumentEnd),
];

const OTHER_PLATFORMS: &[(&str, EditShortcut)] = &[
    ("ctrl-z", EditShortcut::Undo),
    ("ctrl-y", EditShortcut::Redo),
    ("ctrl-shift-z", EditShortcut::Redo),
    ("ctrl-left", EditShortcut::WordLeft),
    ("ctrl-right", EditShortcut::WordRight),
    ("ctrl-shift-left", EditShortcut::SelectWordLeft),
    ("ctrl-shift-right", EditShortcut::SelectWordRight),
    ("ctrl-backspace", EditShortcut::DeleteWordBackward),
    ("ctrl-delete", EditShortcut::DeleteWordForward),
    ("ctrl-home", EditShortcut::DocumentStart),
    ("ctrl-end", EditShortcut::DocumentEnd),
    ("ctrl-shift-home", EditShortcut::SelectDocumentStart),
    ("ctrl-shift-end", EditShortcut::SelectDocumentEnd),
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

/// The key string of the platform this build targets, for tests that drive the keyboard.
#[cfg(test)]
pub(crate) fn platform_keys(macos: &'static str, elsewhere: &'static str) -> &'static str {
    if cfg!(target_os = "macos") {
        macos
    } else {
        elsewhere
    }
}

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

    #[test]
    fn word_motion_and_deletion_use_option_on_macos() {
        let macos = shortcuts(true);
        assert!(macos.contains(&("alt-left", EditShortcut::WordLeft)));
        assert!(macos.contains(&("alt-shift-right", EditShortcut::SelectWordRight)));
        assert!(macos.contains(&("alt-backspace", EditShortcut::DeleteWordBackward)));
        assert!(macos.contains(&("alt-delete", EditShortcut::DeleteWordForward)));
    }

    #[test]
    fn word_motion_and_deletion_use_control_elsewhere() {
        let elsewhere = shortcuts(false);
        assert!(elsewhere.contains(&("ctrl-left", EditShortcut::WordLeft)));
        assert!(elsewhere.contains(&("ctrl-shift-left", EditShortcut::SelectWordLeft)));
        assert!(elsewhere.contains(&("ctrl-backspace", EditShortcut::DeleteWordBackward)));
        assert!(elsewhere.contains(&("ctrl-delete", EditShortcut::DeleteWordForward)));
        assert!(elsewhere.iter().all(|(keys, _)| !keys.starts_with("alt-")));
    }

    #[test]
    fn line_and_document_edges_follow_each_platform() {
        assert!(shortcuts(true).contains(&("cmd-left", EditShortcut::LineStart)));
        assert!(shortcuts(true).contains(&("cmd-up", EditShortcut::DocumentStart)));
        assert!(shortcuts(false).contains(&("ctrl-home", EditShortcut::DocumentStart)));
        assert!(shortcuts(false).contains(&("ctrl-shift-end", EditShortcut::SelectDocumentEnd)));
    }

    #[test]
    fn each_platform_binds_each_key_once() {
        for mac in [true, false] {
            let keys: std::collections::HashSet<&str> =
                shortcuts(mac).iter().map(|(keys, _)| *keys).collect();
            assert_eq!(keys.len(), shortcuts(mac).len(), "a key is bound twice");
        }
    }
}
