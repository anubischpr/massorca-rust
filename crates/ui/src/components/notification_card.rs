//! Notification cards and their stack (docs/design/components/NotificationCard.md): a
//! persistent card that asks for attention and offers at most two actions. The stack shows the
//! three highest-priority cards and folds the rest into a "+N more" row.

use gpui::SharedString;

/// What a card is about. It sets the plate, the glyph and the meta word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationKind {
    Attention,
    Error,
    Update,
    SignIn,
    Info,
}

impl NotificationKind {
    /// The meta word; it is always shown, so the kind never depends on colour alone.
    pub fn word(self) -> &'static str {
        match self {
            Self::Attention => "Waiting for you",
            Self::Error => "Error",
            Self::Update => "Update",
            Self::SignIn => "Sign-in needed",
            Self::Info => "Note",
        }
    }

    /// Lower sorts higher in a stack: attention and errors, then sign-in, updates and notes.
    pub fn priority(self) -> u8 {
        match self {
            Self::Attention | Self::Error => 0,
            Self::SignIn => 1,
            Self::Update => 2,
            Self::Info => 3,
        }
    }
}

/// The update card's state; its title, progress and actions follow it.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Available { version: SharedString },
    Checking,
    Downloading { percent: u8 },
    Ready,
    Failed { reason: SharedString },
}

impl UpdateState {
    /// A failed update is an error; every other state is an update.
    pub fn kind(&self) -> NotificationKind {
        match self {
            Self::Failed { .. } => NotificationKind::Error,
            _ => NotificationKind::Update,
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Available { version } => format!("MassOrca {version} is available"),
            Self::Checking => "Checking for updates".to_owned(),
            Self::Downloading { percent } => format!("Downloading, {percent}%"),
            Self::Ready => "Restart to finish the update".to_owned(),
            Self::Failed { reason } => format!("Update failed: {reason}"),
        }
    }

    /// The bar fraction while downloading.
    pub fn progress(&self) -> Option<f32> {
        match self {
            Self::Downloading { percent } => Some(f32::from(*percent) / 100.0),
            _ => None,
        }
    }

    /// Action labels, primary first; the caller maps each one to a Button.
    pub fn action_labels(&self) -> &'static [&'static str] {
        match self {
            Self::Available { .. } => &["Download", "Later"],
            Self::Ready => &["Restart now", "Later"],
            Self::Failed { .. } => &["Retry"],
            Self::Checking | Self::Downloading { .. } => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_carry_the_spec_words_and_priorities() {
        assert_eq!(NotificationKind::Attention.word(), "Waiting for you");
        assert_eq!(NotificationKind::SignIn.word(), "Sign-in needed");
        assert_eq!(NotificationKind::Info.word(), "Note");
        assert_eq!(
            NotificationKind::Attention.priority(),
            NotificationKind::Error.priority()
        );
        assert!(NotificationKind::Error.priority() < NotificationKind::SignIn.priority());
        assert!(NotificationKind::SignIn.priority() < NotificationKind::Update.priority());
        assert!(NotificationKind::Update.priority() < NotificationKind::Info.priority());
    }

    #[test]
    fn update_states_follow_the_spec_copy() {
        let available = UpdateState::Available {
            version: "1.4.2".into(),
        };
        assert_eq!(available.title(), "MassOrca 1.4.2 is available");
        assert_eq!(available.action_labels(), &["Download", "Later"]);
        let failed = UpdateState::Failed {
            reason: "offline".into(),
        };
        assert_eq!(failed.kind(), NotificationKind::Error);
        assert_eq!(failed.title(), "Update failed: offline");
        assert_eq!(failed.action_labels(), &["Retry"]);
        let downloading = UpdateState::Downloading { percent: 64 };
        assert_eq!(downloading.title(), "Downloading, 64%");
        assert!(
            downloading
                .progress()
                .is_some_and(|f| (f - 0.64).abs() < 1e-6)
        );
        assert!(UpdateState::Checking.action_labels().is_empty());
    }
}
