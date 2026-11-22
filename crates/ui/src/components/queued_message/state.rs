//! The queue's rules without a view: what a waiting message is, how the dock collapses and
//! which row holds an inline step, what the header and captions say, and where focus goes after
//! a removal.

use gpui::SharedString;

/// Why a queued message waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitReason {
    /// The agent is busy with the run before it.
    AgentBusy,
    /// The host has not confirmed the session state; it sends when the host reconnects.
    HostUnconfirmed,
}

/// Where a queued message is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueStatus {
    Waiting(WaitReason),
    Sending,
    /// The send failed for this reason; the message stays queued.
    Failed(SharedString),
}

/// A queued message as the session holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedItem {
    pub id: u64,
    pub text: SharedString,
    pub status: QueueStatus,
}

/// The inline step a row is in; only one row can be in one at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowStep {
    /// "Send now?" asks before the run is interrupted.
    Confirm,
    /// The text is an inline editor.
    Edit,
}

/// The dock's own state: whether the list is expanded, and the one row in an inline step.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueueView {
    expanded: bool,
    step: Option<(u64, RowStep)>,
}

impl QueueView {
    /// More rows than this collapse to `SHOWN_WHEN_COLLAPSED` and a "+N more" row.
    pub const COLLAPSE_ABOVE: usize = 3;
    pub const SHOWN_WHEN_COLLAPSED: usize = 2;

    pub fn expanded(&self) -> bool {
        self.expanded
    }

    pub fn step(&self) -> Option<(u64, RowStep)> {
        self.step
    }

    /// The rows drawn for a queue of `len` messages.
    pub fn visible(&self, len: usize) -> usize {
        if self.expanded || len <= Self::COLLAPSE_ABOVE {
            len
        } else {
            Self::SHOWN_WHEN_COLLAPSED
        }
    }

    pub fn hidden(&self, len: usize) -> usize {
        len - self.visible(len)
    }

    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }

    pub fn expand(&mut self) {
        self.expanded = true;
    }

    /// Starts an inline step on a row, replacing any step already open.
    pub fn begin(&mut self, id: u64, step: RowStep) {
        self.step = Some((id, step));
    }

    pub fn end_step(&mut self) {
        self.step = None;
    }

    /// Drops the step when its row has left the queue.
    pub fn retain(&mut self, ids: &[u64]) {
        if self.step.is_some_and(|(id, _)| !ids.contains(&id)) {
            self.step = None;
        }
    }
}

/// The header above the rows: "Queued (n)", shown from two messages.
pub fn header_label(len: usize) -> Option<String> {
    (len >= 2).then(|| format!("Queued ({len})"))
}

/// The caption under a message's text, saying why it waits or what went wrong.
pub fn caption(status: &QueueStatus) -> SharedString {
    match status {
        QueueStatus::Waiting(WaitReason::AgentBusy) => "Waits for the agent".into(),
        QueueStatus::Waiting(WaitReason::HostUnconfirmed) => {
            "Waits for VPS to confirm. Sends when it reconnects.".into()
        }
        QueueStatus::Sending => "Sending".into(),
        QueueStatus::Failed(reason) => format!("Not sent: {reason}").into(),
    }
}

/// Where focus goes once the row at `removed` has left a list of `len_after` rows: the row that
/// moved into its place, else the one before it, else nowhere (back to the composer).
pub fn focus_after_removal(len_after: usize, removed: usize) -> Option<usize> {
    if removed < len_after {
        Some(removed)
    } else {
        len_after.checked_sub(1)
    }
}

/// What the dock reports to its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueuedMessagesEvent {
    /// The user confirmed Send now for this message.
    SendNow(u64),
    /// The user saved an edited text for this message.
    Edited { id: u64, text: SharedString },
    /// The user removed this message; the owner removes it and may offer Undo.
    Removed(u64),
    /// The user asked to send a failed message again.
    Retry(u64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn more_than_three_rows_collapse_to_two_and_a_more_row() {
        let mut view = QueueView::default();
        assert_eq!((view.visible(3), view.hidden(3)), (3, 0));
        assert_eq!((view.visible(5), view.hidden(5)), (2, 3));
        view.expand();
        assert_eq!((view.visible(5), view.hidden(5)), (5, 0));
    }

    #[test]
    fn the_header_counts_from_two_messages() {
        assert_eq!(header_label(1), None);
        assert_eq!(header_label(2).as_deref(), Some("Queued (2)"));
    }

    #[test]
    fn captions_say_why_a_message_waits_or_failed() {
        assert_eq!(
            caption(&QueueStatus::Waiting(WaitReason::AgentBusy)).as_ref(),
            "Waits for the agent"
        );
        assert_eq!(
            caption(&QueueStatus::Waiting(WaitReason::HostUnconfirmed)).as_ref(),
            "Waits for VPS to confirm. Sends when it reconnects."
        );
        assert_eq!(caption(&QueueStatus::Sending).as_ref(), "Sending");
        assert_eq!(
            caption(&QueueStatus::Failed("timeout".into())).as_ref(),
            "Not sent: timeout"
        );
    }

    #[test]
    fn focus_moves_to_the_next_row_then_the_previous_then_the_composer() {
        assert_eq!(focus_after_removal(3, 1), Some(1), "the row that moved up");
        assert_eq!(focus_after_removal(2, 2), Some(1), "the previous row");
        assert_eq!(focus_after_removal(0, 0), None, "back to the composer");
    }

    #[test]
    fn a_step_on_a_row_that_left_the_queue_is_dropped() {
        let mut view = QueueView::default();
        view.begin(4, RowStep::Confirm);
        view.retain(&[1, 2]);
        assert_eq!(view.step(), None);
        view.begin(2, RowStep::Edit);
        view.retain(&[1, 2]);
        assert_eq!(view.step(), Some((2, RowStep::Edit)));
    }
}
