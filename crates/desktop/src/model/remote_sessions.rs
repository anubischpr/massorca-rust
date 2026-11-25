//! The agent sessions each paired host runs, as Sidebar rows under the host's row (STAT-16, A97), and the view
//! of one of them. A host that the newest read did not reach keeps its last sessions, dimmed and unverifiable,
//! never exited. Free of GPUI, so the rules test without a window.

use super::agent_sessions::host_session_rows;
use super::shell::{AgentState, SessionRow};
use super::sidebar::{TreeRow, tooltip_for};
use crate::service::{HostSessions, StoreState};

/// One paired host's sessions as the Sidebar lists them: the top sessions, each with its children below it.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteSessions {
    /// Whether the newest read reached the host. While false, every row is unverifiable and dimmed.
    pub reached: bool,
    pub rows: Vec<SessionRow>,
}

/// One remote session as its view shows it: its header, its status and the host it runs on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteSessionView {
    pub host: String,
    pub session: String,
    pub title: String,
    /// The agent kind the host reports for the session, such as `claude`.
    pub kind: String,
    pub state: AgentState,
    pub reached: bool,
}

/// Each paired host's sessions, by host name, for the hosts the store has checked at least once.
pub fn remote_sessions(state: &StoreState) -> Vec<(String, RemoteSessions)> {
    state
        .host_sessions
        .iter()
        .map(|(name, held)| (name.clone(), sessions_of(held)))
        .collect()
}

/// The row of session `id` on any paired host, with the host's name.
pub fn remote_row<'a>(
    remote: &'a [(String, RemoteSessions)],
    id: &str,
) -> Option<(&'a str, &'a SessionRow)> {
    remote
        .iter()
        .find_map(|(host, sessions)| find_in(&sessions.rows, id).map(|row| (host.as_str(), row)))
}

fn find_in<'a>(rows: &'a [SessionRow], id: &str) -> Option<&'a SessionRow> {
    rows.iter().find_map(|row| {
        if row.id == id {
            Some(row)
        } else {
            find_in(&row.children, id)
        }
    })
}

/// The view of session `id` on `host`, as the store holds it now. `None` when the host does not list it.
pub fn remote_session_view(state: &StoreState, host: &str, id: &str) -> Option<RemoteSessionView> {
    let held = state.host_sessions.get(host)?;
    let sessions = sessions_of(held);
    let row = find_in(&sessions.rows, id)?;
    let kind = held
        .sessions
        .iter()
        .find(|session| session.id == id)
        .map(|session| session.kind.clone())?;
    Some(RemoteSessionView {
        host: host.to_owned(),
        session: row.id.clone(),
        title: row.title.clone(),
        kind,
        state: row.state,
        reached: held.reached,
    })
}

fn sessions_of(held: &HostSessions) -> RemoteSessions {
    let mut rows = host_session_rows(&held.sessions, &held.workspaces);
    for row in &mut rows {
        settle(row, held.reached);
    }
    RemoteSessions {
        reached: held.reached,
        rows,
    }
}

/// A row of a host's sessions: a host the read did not reach shows unverifiable, not exited. Bring back works only
/// on This PC, so no row of another host offers it.
fn settle(row: &mut SessionRow, reached: bool) {
    row.can_bring_back = false;
    if !reached {
        row.state = AgentState::Unreachable;
    }
    for child in &mut row.children {
        settle(child, reached);
    }
}

/// The Sidebar rows of a host's sessions, below its host row: each top session, then its children by depth (A97).
pub fn session_tree_rows(
    host: &str,
    sessions: &RemoteSessions,
    active: Option<&str>,
) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    for row in &sessions.rows {
        push_tree_row(&mut rows, host, row, 0, active, !sessions.reached);
    }
    rows
}

fn push_tree_row(
    rows: &mut Vec<TreeRow>,
    host: &str,
    row: &SessionRow,
    depth: usize,
    active: Option<&str>,
    dimmed: bool,
) {
    rows.push(TreeRow::Session {
        id: row.id.clone(),
        title: row.title.clone(),
        agent: row.agent,
        state: row.state,
        depth,
        selected: active == Some(row.id.as_str()),
        unread: false,
        can_bring_back: false,
        tooltip: tooltip_for(row.branch.as_deref(), row.path.as_deref()),
        workspace_id: row.workspace_id.clone(),
        dimmed,
        local: false,
        host: Some(host.to_owned()),
    });
    for child in &row.children {
        push_tree_row(rows, host, child, depth + 1, active, dimmed);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use massorca_protocol::methods::agents::AgentSession;
    use massorca_protocol::methods::terminals::TerminalState;

    use super::*;
    use crate::model::shell::AgentState;
    use crate::service::fixtures::{agent, workspace};
    use crate::service::{HostAgents, HostSessionsRead, StoreUpdate};

    fn answer(host: &str, request: u64, sessions: Vec<AgentSession>) -> StoreUpdate {
        StoreUpdate::HostSessions(HostSessionsRead {
            host: host.to_owned(),
            request,
            answer: Some(HostAgents {
                sessions,
                workspaces: vec![workspace("w1", "p1", "feat", 1)],
            }),
        })
    }

    fn unreached(host: &str, request: u64) -> StoreUpdate {
        StoreUpdate::HostSessions(HostSessionsRead {
            host: host.to_owned(),
            request,
            answer: None,
        })
    }

    fn exited(mut session: AgentSession) -> AgentSession {
        session.execution.state = TerminalState::Exited;
        session
    }

    fn ids_of(state: &StoreState, host: &str) -> Vec<String> {
        remote_sessions(state)
            .into_iter()
            .find(|(name, _)| name == host)
            .map(|(_, sessions)| sessions.rows.iter().map(|row| row.id.clone()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn each_host_lists_only_the_sessions_it_runs() {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(answer("vps", 1, vec![agent("a1", "w1", 1)]), now);
        state.apply(answer("gpu", 1, vec![agent("a2", "w1", 2)]), now);
        assert_eq!(ids_of(&state, "vps"), vec!["a1"]);
        assert_eq!(ids_of(&state, "gpu"), vec!["a2"]);
    }

    #[test]
    fn a_host_never_checked_lists_no_sessions() {
        let state = StoreState::default();
        assert!(remote_sessions(&state).is_empty());
        assert!(ids_of(&state, "vps").is_empty());
    }

    #[test]
    fn a_host_that_stops_answering_keeps_its_sessions_as_unverifiable_and_dimmed_never_exited() {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(answer("vps", 1, vec![exited(agent("a1", "w1", 1))]), now);
        let (_, reached) = &remote_sessions(&state)[0];
        assert_eq!(
            reached.rows[0].state,
            AgentState::Done,
            "reached: the run's end is known"
        );

        state.apply(unreached("vps", 2), now);
        let (_, sessions) = &remote_sessions(&state)[0];
        assert!(!sessions.reached);
        assert_eq!(
            sessions.rows.len(),
            1,
            "the last known session stays listed"
        );
        assert_eq!(sessions.rows[0].state, AgentState::Unreachable);
        let tree = session_tree_rows("vps", sessions, None);
        assert!(matches!(
            &tree[0],
            TreeRow::Session {
                id,
                state: AgentState::Unreachable,
                dimmed: true,
                local: false,
                ..
            } if id == "a1"
        ));
    }

    #[test]
    fn a_reconnect_shows_the_newest_check_and_an_older_answer_changes_nothing() {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(answer("vps", 1, vec![agent("a1", "w1", 1)]), now);
        state.apply(unreached("vps", 2), now);
        state.apply(
            answer("vps", 4, vec![agent("a1", "w1", 1), agent("a2", "w1", 2)]),
            now,
        );
        // A late answer of an older check, which says the host is gone, does not undo the reconnect.
        state.apply(unreached("vps", 3), now);
        let (_, sessions) = &remote_sessions(&state)[0];
        assert!(sessions.reached);
        assert_eq!(sessions.rows.len(), 2);
        assert!(session_tree_rows("vps", sessions, None).iter().all(|row| {
            matches!(
                row,
                TreeRow::Session {
                    dimmed: false,
                    can_bring_back: false,
                    ..
                }
            )
        }));
    }

    #[test]
    fn a_session_row_of_another_host_is_never_offered_bring_back() {
        let now = Instant::now();
        let mut state = StoreState::default();
        let mut parent = agent("p", "w1", 1);
        parent.title = Some("Plan".into());
        let mut child = exited(agent("c", "w1", 2));
        child.parent_id = Some("p".into());
        state.apply(answer("vps", 1, vec![parent, child]), now);
        let (_, sessions) = &remote_sessions(&state)[0];
        assert_eq!(sessions.rows[0].children.len(), 1);
        let tree = session_tree_rows("vps", sessions, Some("c"));
        assert!(matches!(
            &tree[1],
            TreeRow::Session { id, depth: 1, selected: true, can_bring_back: false, host: Some(host), .. }
                if id == "c" && host == "vps"
        ));
    }

    #[test]
    fn a_remote_session_view_carries_its_host_kind_and_last_known_state() {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(answer("vps", 1, vec![agent("a1", "w1", 1)]), now);
        let view = remote_session_view(&state, "vps", "a1").expect("the host lists it");
        assert_eq!(view.host, "vps");
        assert_eq!(view.kind, "claude");
        assert!(view.reached);
        state.apply(unreached("vps", 2), now);
        let view = remote_session_view(&state, "vps", "a1").expect("still listed");
        assert!(!view.reached);
        assert_eq!(view.state, AgentState::Unreachable);
        assert!(remote_session_view(&state, "vps", "missing").is_none());
        assert!(remote_session_view(&state, "gpu", "a1").is_none());
    }

    #[test]
    fn a_row_is_found_on_the_host_that_lists_it() {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(answer("vps", 1, vec![agent("a1", "w1", 1)]), now);
        state.apply(answer("gpu", 1, vec![agent("a2", "w1", 2)]), now);
        let listed = remote_sessions(&state);
        let (host, row) = remote_row(&listed, "a2").expect("gpu lists it");
        assert_eq!((host, row.id.as_str()), ("gpu", "a2"));
        assert!(remote_row(&listed, "nope").is_none());
    }
}
