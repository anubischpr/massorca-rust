//! The composer's rules without a window: what the primary button does, what Enter and
//! Escape mean, which sent messages Up and Down recall, and how attachments and names fit.

/// What the composer's state allows right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Situation {
    /// The agent is running a turn.
    pub agent_busy: bool,
    /// The host has confirmed the session state; false while it is unreachable.
    pub host_confirmed: bool,
    /// The session is archived or cannot take input, or sending waits (sign-in, no agent yet).
    pub disabled: bool,
    pub text_empty: bool,
    /// An IME composition is active; Enter confirms the composition instead.
    pub composing: bool,
    /// A message sent while the agent runs queues behind its turn; otherwise the button stops the run.
    pub queueing: bool,
}

/// What the primary button in the bottom row does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimaryAction {
    Send,
    /// Queue the message behind the running turn.
    Queue,
    /// Stop the running turn (text empty, agent busy).
    Stop,
    Disabled,
}

/// The primary button: Stop when there is nothing to send and the agent runs, or while it runs
/// when the composer cannot queue; otherwise Send, or Queue while the agent runs. Send is disabled
/// while the host is unreachable (the message stays in the composer and is not queued).
pub fn primary_action(situation: Situation) -> PrimaryAction {
    if situation.agent_busy && (situation.text_empty || !situation.queueing) {
        return PrimaryAction::Stop;
    }
    if situation.text_empty {
        return PrimaryAction::Disabled;
    }
    if situation.disabled || !situation.host_confirmed {
        return PrimaryAction::Disabled;
    }
    if situation.agent_busy {
        PrimaryAction::Queue
    } else {
        PrimaryAction::Send
    }
}

/// What an Enter key press does. `modified` is Mod+Enter; Shift+Enter is a line break in the
/// text area itself and never reaches this rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnterAction {
    Send,
    Queue,
    /// Insert a line break (the "Enter inserts a new line" setting).
    Newline,
    Nothing,
}

pub fn enter_action(
    situation: Situation,
    modified: bool,
    enter_inserts_newline: bool,
) -> EnterAction {
    if situation.composing {
        return EnterAction::Nothing;
    }
    if !modified && enter_inserts_newline {
        return EnterAction::Newline;
    }
    match primary_action(situation) {
        PrimaryAction::Send => EnterAction::Send,
        PrimaryAction::Queue => EnterAction::Queue,
        PrimaryAction::Stop | PrimaryAction::Disabled => EnterAction::Nothing,
    }
}

/// What an Escape key press does: while the agent runs with nothing typed, the first press
/// shows "Press Escape again to stop" and the second stops the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeAction {
    Nothing,
    ShowHint,
    Stop,
}

pub fn escape_action(agent_busy: bool, text_empty: bool, hint_shown: bool) -> EscapeAction {
    match (agent_busy && text_empty, hint_shown) {
        (false, _) => EscapeAction::Nothing,
        (true, false) => EscapeAction::ShowHint,
        (true, true) => EscapeAction::Stop,
    }
}

/// The messages sent in this composer, for Up (earlier) and Down (later).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SentHistory {
    entries: Vec<String>,
    cursor: Option<usize>,
}

impl SentHistory {
    /// Records a sent message; blank text is not history.
    pub fn push(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        self.entries.push(text.to_owned());
        self.cursor = None;
    }

    /// Up: the message before the one shown, or the newest one first.
    pub fn older(&mut self) -> Option<String> {
        let next = match self.cursor {
            None => self.entries.len().checked_sub(1)?,
            Some(index) => index.checked_sub(1)?,
        };
        self.cursor = Some(next);
        self.entries.get(next).cloned()
    }

    /// Down: the message after the one shown; past the newest, an empty draft.
    pub fn newer(&mut self) -> Option<String> {
        let index = self.cursor?;
        if index + 1 < self.entries.len() {
            self.cursor = Some(index + 1);
            self.entries.get(index + 1).cloned()
        } else {
            self.cursor = None;
            Some(String::new())
        }
    }
}

/// How many attachment chips are drawn: up to four, then three and a "+N" chip.
pub fn visible_attachments(count: usize) -> (usize, usize) {
    if count <= 4 {
        (count, 0)
    } else {
        (3, count - 3)
    }
}

/// A chip name cut to `max` characters with an ellipsis; the full name is the tooltip.
pub fn truncate_name(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        return name.to_owned();
    }
    let kept: String = name.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(agent_busy: bool, host_confirmed: bool, text_empty: bool) -> Situation {
        Situation {
            agent_busy,
            host_confirmed,
            disabled: false,
            text_empty,
            composing: false,
            queueing: true,
        }
    }

    #[test]
    fn the_primary_button_follows_busy_host_and_text() {
        assert_eq!(primary_action(at(false, true, false)), PrimaryAction::Send);
        assert_eq!(primary_action(at(true, true, false)), PrimaryAction::Queue);
        assert_eq!(primary_action(at(true, true, true)), PrimaryAction::Stop);
        assert_eq!(
            primary_action(at(false, true, true)),
            PrimaryAction::Disabled
        );
        assert_eq!(
            primary_action(at(false, false, false)),
            PrimaryAction::Disabled,
            "an unreachable host keeps the message in the composer"
        );
        let archived = Situation {
            disabled: true,
            ..at(false, true, false)
        };
        assert_eq!(primary_action(archived), PrimaryAction::Disabled);
    }

    #[test]
    fn a_composer_that_cannot_queue_stops_a_running_agent_whatever_the_draft_holds() {
        let running = |text_empty| Situation {
            queueing: false,
            ..at(true, true, text_empty)
        };
        assert_eq!(primary_action(running(true)), PrimaryAction::Stop);
        assert_eq!(primary_action(running(false)), PrimaryAction::Stop);
        assert_eq!(
            enter_action(running(false), false, false),
            EnterAction::Nothing,
            "Enter neither sends nor queues into a running agent"
        );
    }

    #[test]
    fn plain_enter_sends_or_queues_and_mod_enter_does_the_same() {
        assert_eq!(
            enter_action(at(false, true, false), false, false),
            EnterAction::Send
        );
        assert_eq!(
            enter_action(at(true, true, false), false, false),
            EnterAction::Queue
        );
        assert_eq!(
            enter_action(at(false, true, false), true, false),
            EnterAction::Send
        );
        assert_eq!(
            enter_action(at(false, true, true), false, false),
            EnterAction::Nothing
        );
    }

    #[test]
    fn the_newline_setting_swaps_plain_enter_and_mod_enter() {
        assert_eq!(
            enter_action(at(false, true, false), false, true),
            EnterAction::Newline
        );
        assert_eq!(
            enter_action(at(false, true, false), true, true),
            EnterAction::Send
        );
    }

    #[test]
    fn enter_during_composition_never_sends() {
        let composing = Situation {
            composing: true,
            ..at(false, true, false)
        };
        assert_eq!(enter_action(composing, false, false), EnterAction::Nothing);
        assert_eq!(enter_action(composing, true, false), EnterAction::Nothing);
    }

    #[test]
    fn escape_hints_first_and_stops_on_the_second_press_only_while_busy_and_empty() {
        assert_eq!(escape_action(true, true, false), EscapeAction::ShowHint);
        assert_eq!(escape_action(true, true, true), EscapeAction::Stop);
        assert_eq!(escape_action(true, false, false), EscapeAction::Nothing);
        assert_eq!(escape_action(false, true, false), EscapeAction::Nothing);
    }

    #[test]
    fn up_recalls_the_newest_message_then_earlier_ones_and_down_moves_back() {
        let mut history = SentHistory::default();
        history.push("first");
        history.push("   ");
        history.push("second");
        assert_eq!(history.older().as_deref(), Some("second"));
        assert_eq!(history.older().as_deref(), Some("first"));
        assert_eq!(history.older(), None, "the oldest stays");
        assert_eq!(history.newer().as_deref(), Some("second"));
        assert_eq!(
            history.newer().as_deref(),
            Some(""),
            "past the newest, an empty draft"
        );
        assert_eq!(history.newer(), None);
    }

    #[test]
    fn a_big_attachment_list_collapses_to_three_and_a_count() {
        assert_eq!(visible_attachments(4), (4, 0));
        assert_eq!(visible_attachments(6), (3, 3));
    }

    #[test]
    fn long_names_cut_at_the_limit_with_an_ellipsis() {
        assert_eq!(truncate_name("notes.md", 24), "notes.md");
        let cut = truncate_name("a-very-long-attachment-name-indeed.pdf", 24);
        assert_eq!(cut.chars().count(), 24);
        assert!(cut.ends_with('…'));
    }
}
