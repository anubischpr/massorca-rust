//! What closing the main window would leave running (PLAT-10). The service keeps running after the window closes,
//! so a close asks first only when sessions are running or waiting for the person. Plain functions over the store's
//! sessions, so tests call them without GPUI (decision 0028).

use massorca_protocol::methods::agents::AgentSession;

use super::agent_sessions::session_state;
use super::shell::AgentState;

/// What a close would leave running in the service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseRisk {
    /// Sessions running or waiting for the person, children included.
    pub running_sessions: usize,
}

/// What closing now would put at stake, or None when no session is running or waiting and the window closes at once.
pub fn close_check(sessions: &[AgentSession]) -> Option<CloseRisk> {
    let running_sessions = sessions
        .iter()
        .filter(|session| {
            matches!(
                session_state(session),
                AgentState::Running | AgentState::Waiting
            )
        })
        .count();
    (running_sessions > 0).then_some(CloseRisk { running_sessions })
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::agents::{Activity, ActivityState, AgentSession, Outcome};
    use massorca_protocol::methods::terminals::TerminalState;

    use super::{CloseRisk, close_check};
    use crate::service::fixtures::agent;

    /// A live session whose agent reports `state`, with `outcome` once it is done.
    fn reporting(id: &str, state: ActivityState, outcome: Option<Outcome>) -> AgentSession {
        let mut session = agent(id, "w1", 10);
        session.activity = Some(Activity {
            state,
            outcome,
            at_ms: 5,
            ..crate::service::fixtures::base_activity()
        });
        session
    }

    #[test]
    fn nothing_is_at_stake_when_no_session_is_running_or_waiting() {
        let mut exited_while_working = reporting("x", ActivityState::Working, None);
        exited_while_working.execution.state = TerminalState::Exited;
        let mut unreachable = agent("u", "w1", 10);
        unreachable.execution.state = TerminalState::Unverifiable;
        let sessions = [
            agent("not-started", "w1", 5),
            reporting("idle", ActivityState::Idle, None),
            reporting("failed", ActivityState::Done, Some(Outcome::Failure)),
            exited_while_working,
            unreachable,
        ];
        assert_eq!(close_check(&sessions), None);
        assert_eq!(close_check(&[]), None);
    }

    #[test]
    fn running_and_waiting_sessions_are_counted_children_included() {
        let mut child = reporting("child", ActivityState::Working, None);
        child.parent_id = Some("parent".into());
        let sessions = [
            reporting("a", ActivityState::Working, None),
            reporting("b", ActivityState::Waiting, None),
            child,
            reporting("idle", ActivityState::Idle, None),
        ];
        assert_eq!(
            close_check(&sessions),
            Some(CloseRisk {
                running_sessions: 3
            })
        );
    }
}
