//! The wait a parent session is in while a child writes in the parent's own worktree (decision 0170): the parent
//! may not write there until the child finishes. Plain functions over the store, so tests call them without GPUI.

use massorca_protocol::methods::agents::AgentSession;

use super::agent_sessions::session_state;
use super::shell::AgentState;

/// The child a parent waits for, as the parent's strip names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildWait {
    /// The child's session id, which the strip's Open child button opens.
    pub child: String,
    /// The child's agent kind, named in the strip.
    pub kind: String,
    /// When the child started, in milliseconds since the Unix epoch.
    pub started_at_ms: i64,
}

/// The oldest unfinished session of `parent` that writes in the parent's own workspace, or None.
pub fn child_wait(sessions: &[AgentSession], parent: &AgentSession) -> Option<ChildWait> {
    sessions
        .iter()
        .filter(|session| session.parent_id.as_deref() == Some(parent.id.as_str()))
        .filter(|session| !session.read_only && session.workspace_id == parent.workspace_id)
        .filter(|session| !matches!(session_state(session), AgentState::Done | AgentState::Error))
        .min_by_key(|session| session.created_at_ms)
        .map(|session| ChildWait {
            child: session.id.clone(),
            kind: session.kind.clone(),
            started_at_ms: session.created_at_ms,
        })
}

/// How long something has run, in words: "under a minute", "4 min", "1 h 5 min".
pub fn elapsed_words(started_at_ms: i64, now_ms: i64) -> String {
    let minutes = now_ms.saturating_sub(started_at_ms).max(0) / 60_000;
    if minutes == 0 {
        "under a minute".to_owned()
    } else if minutes < 60 {
        format!("{minutes} min")
    } else {
        format!("{} h {} min", minutes / 60, minutes % 60)
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::agents::{Activity, ActivityState, AgentSession, Outcome};

    use super::{ChildWait, child_wait, elapsed_words};
    use crate::service::fixtures::{agent, base_activity};

    fn child_of(parent: &str, id: &str, workspace: &str, created: i64) -> AgentSession {
        let mut session = agent(id, workspace, created);
        session.parent_id = Some(parent.into());
        session.activity = Some(Activity {
            state: ActivityState::Working,
            ..base_activity()
        });
        session
    }

    #[test]
    fn a_writing_child_in_the_parent_worktree_holds_the_parent_until_it_ends() {
        let parent = agent("p", "w1", 10);
        let writer = child_of("p", "c", "w1", 20);
        assert_eq!(
            child_wait(&[parent.clone(), writer], &parent),
            Some(ChildWait {
                child: "c".into(),
                kind: "claude".into(),
                started_at_ms: 20,
            })
        );
    }

    #[test]
    fn a_read_only_child_a_child_in_its_own_worktree_and_a_finished_child_leave_the_parent_free() {
        let parent = agent("p", "w1", 10);
        let mut reader = child_of("p", "r", "w1", 20);
        reader.read_only = true;
        let own_worktree = child_of("p", "o", "w2", 21);
        let mut finished = child_of("p", "f", "w1", 22);
        finished.activity = Some(Activity {
            state: ActivityState::Done,
            outcome: Some(Outcome::Success),
            at_ms: 5,
            ..base_activity()
        });
        let sessions = [parent.clone(), reader, own_worktree, finished];
        assert_eq!(child_wait(&sessions, &parent), None);
    }

    #[test]
    fn the_oldest_unfinished_writing_child_is_the_one_waited_for() {
        let parent = agent("p", "w1", 10);
        let newer = child_of("p", "n", "w1", 30);
        let older = child_of("p", "o", "w1", 20);
        let waited = child_wait(&[parent.clone(), newer, older], &parent);
        assert_eq!(waited.map(|wait| wait.child), Some("o".to_owned()));
    }

    #[test]
    fn the_elapsed_time_is_said_in_minutes_and_then_hours() {
        assert_eq!(elapsed_words(0, 30_000), "under a minute");
        assert_eq!(elapsed_words(0, 4 * 60_000), "4 min");
        assert_eq!(elapsed_words(0, 65 * 60_000), "1 h 5 min");
    }
}
