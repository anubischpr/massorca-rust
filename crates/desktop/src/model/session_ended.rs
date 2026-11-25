//! The ended state of an agent session: what its session view offers once the run has ended, and when a refused
//! resume means the saved conversation is gone. Pure policy; the session view draws it.

use massorca_protocol::codes;
use massorca_protocol::methods::agents::AgentSession;

/// The reason an ended session that can resume shows beside Resume.
pub const ENDED: &str = "This session has ended.";
/// The reason an ended session of a kind that cannot resume shows.
pub const ENDED_NOT_RESUMABLE: &str = "This session has ended and cannot be resumed.";
/// The reason an ended session shows once its saved conversation is known to be gone.
pub const CONVERSATION_NOT_FOUND: &str = "The saved conversation could not be found.";
/// The action that starts again in the same workspace with the same agent, without the saved conversation.
pub const START_NEW: &str = "Start a new session here";
/// The refusal of `agent.resume` when the vendor saved no conversation. The protocol does not name it yet.
pub const RESUME_NOT_FOUND: &str = "agent.resume_not_found";
/// What the vendor CLI prints, then exits, when its saved conversation is missing.
const NO_CONVERSATION: &str = "No conversation found";

/// What an ended session offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndedOffer {
    /// Why the session cannot simply continue, in plain words.
    pub reason: &'static str,
    /// Resume continues the saved conversation.
    pub resume: bool,
    /// Start a new session here: the same workspace and agent, without the saved conversation.
    pub start_new: bool,
}

/// The offer of an ended session, given whether its kind can resume and whether its saved conversation is gone.
pub fn ended_offer(resumable: bool, conversation_gone: bool) -> EndedOffer {
    match (resumable, conversation_gone) {
        (_, true) => EndedOffer {
            reason: CONVERSATION_NOT_FOUND,
            resume: false,
            start_new: true,
        },
        (true, false) => EndedOffer {
            reason: ENDED,
            resume: true,
            start_new: false,
        },
        (false, false) => EndedOffer {
            reason: ENDED_NOT_RESUMABLE,
            resume: false,
            start_new: true,
        },
    }
}

/// Whether a refused `agent.resume` means the saved conversation is gone, rather than a refusal to retry.
pub fn refusal_says_conversation_gone(code: Option<&str>) -> bool {
    matches!(
        code,
        Some(codes::AGENT_RESUME_UNAVAILABLE | RESUME_NOT_FOUND)
    )
}

/// Whether a terminal's screen rows show the vendor CLI saying its saved conversation is missing.
pub fn screen_says_conversation_gone(rows: impl IntoIterator<Item = impl AsRef<str>>) -> bool {
    rows.into_iter()
        .any(|row| row.as_ref().contains(NO_CONVERSATION))
}

/// Whether `terminal` is the terminal of the run an agent session holds now. Such a run's exit is answered by its
/// session view, so its pane offers no Restart, which would start a plain terminal instead.
pub fn is_agent_run_terminal(agents: &[AgentSession], terminal: &str) -> bool {
    agents
        .iter()
        .any(|session| session.terminal_id.as_deref() == Some(terminal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::agent;

    #[test]
    fn a_resumable_ended_session_offers_resume_and_a_non_resumable_one_a_new_start() {
        assert_eq!(
            ended_offer(true, false),
            EndedOffer {
                reason: ENDED,
                resume: true,
                start_new: false,
            }
        );
        assert_eq!(
            ended_offer(false, false),
            EndedOffer {
                reason: ENDED_NOT_RESUMABLE,
                resume: false,
                start_new: true,
            }
        );
    }

    #[test]
    fn a_session_whose_conversation_is_gone_says_so_and_offers_only_a_new_start() {
        for resumable in [true, false] {
            assert_eq!(
                ended_offer(resumable, true),
                EndedOffer {
                    reason: CONVERSATION_NOT_FOUND,
                    resume: false,
                    start_new: true,
                }
            );
        }
    }

    #[test]
    fn a_refused_resume_says_the_conversation_is_gone_only_for_the_two_codes() {
        assert!(refusal_says_conversation_gone(Some(RESUME_NOT_FOUND)));
        assert!(refusal_says_conversation_gone(Some(
            codes::AGENT_RESUME_UNAVAILABLE
        )));
        assert!(!refusal_says_conversation_gone(Some(codes::AGENT_BUSY)));
        assert!(!refusal_says_conversation_gone(None));
    }

    #[test]
    fn the_cli_saying_no_conversation_was_found_is_the_same_conclusion() {
        assert!(screen_says_conversation_gone([
            "$ claude --resume s",
            "No conversation found with session ID: s"
        ]));
        assert!(!screen_says_conversation_gone(["$ ls", "README.md"]));
    }

    #[test]
    fn only_the_terminal_an_agent_session_holds_is_an_agent_run() {
        let mut session = agent("s1", "w1", 1);
        session.terminal_id = Some("t-run".to_owned());
        let agents = [session];
        assert!(is_agent_run_terminal(&agents, "t-run"));
        assert!(!is_agent_run_terminal(&agents, "t-plain"));
        assert!(!is_agent_run_terminal(&[], "t-run"));
    }
}
