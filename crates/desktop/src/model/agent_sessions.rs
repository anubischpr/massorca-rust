//! What the Sidebar shows for the agent sessions the service reported: each session's status, read from
//! its run and from the activity its agent reported last, and the workspace it is listed under. Plain
//! functions over the store, so tests call them without GPUI (decision 0028).

use std::collections::BTreeMap;

use massorca_protocol::methods::agents::{Activity, ActivityState, AgentSession, Outcome};
use massorca_protocol::methods::terminals::TerminalState;

use super::shell::{Agent, AgentState, SessionRow};
use crate::service::StoreState;

/// The title of a session the service has no title for, such as a delegated child.
const UNTITLED: &str = "Untitled session";

/// The sessions of each workspace as rows, oldest first. A session is listed under its own workspace.
pub fn sessions_by_workspace(state: &StoreState) -> BTreeMap<String, Vec<SessionRow>> {
    let mut by_workspace: BTreeMap<String, Vec<SessionRow>> = BTreeMap::new();
    for session in &state.agents {
        by_workspace
            .entry(session.workspace_id.clone())
            .or_default()
            .push(session_row(session));
    }
    by_workspace
}

fn session_row(session: &AgentSession) -> SessionRow {
    SessionRow {
        id: session.id.clone(),
        title: session.title.clone().unwrap_or_else(|| UNTITLED.to_owned()),
        agent: agent_of_kind(&session.kind),
        state: session_state(session),
        children: Vec::new(),
    }
}

/// The status a session shows: how its run stands, and what its agent reported last (DelegationTree).
pub fn session_state(session: &AgentSession) -> AgentState {
    let activity = session.activity.as_ref();
    match session.execution.state {
        TerminalState::Live => live_state(activity),
        TerminalState::Exited => ended_state(activity),
        // The host cannot prove the run is alive or gone, and a lost link is never read as done.
        TerminalState::Unverifiable | TerminalState::Unknown => AgentState::Unreachable,
    }
}

/// A live run shows what its agent reports now. Without a report it has not started yet.
fn live_state(activity: Option<&Activity>) -> AgentState {
    match activity.map(|activity| (activity.state, activity.outcome)) {
        Some((ActivityState::Working, _)) => AgentState::Running,
        Some((ActivityState::Waiting, _)) => AgentState::Waiting,
        Some((ActivityState::Done, outcome)) => done_state(outcome),
        Some((ActivityState::Idle | ActivityState::Unknown, _)) | None => AgentState::Idle,
    }
}

/// An ended run keeps how its last turn ended. A turn the end of the run cut short is done, not running.
fn ended_state(activity: Option<&Activity>) -> AgentState {
    match activity.map(|activity| (activity.state, activity.outcome)) {
        Some((ActivityState::Done, outcome)) => done_state(outcome),
        _ => AgentState::Done,
    }
}

fn done_state(outcome: Option<Outcome>) -> AgentState {
    if outcome == Some(Outcome::Failure) {
        AgentState::Error
    } else {
        AgentState::Done
    }
}

/// The mark of a session's agent. A kind the desktop has no mark for is `Agent::Unknown`.
pub fn agent_of_kind(kind: &str) -> Agent {
    match kind {
        "claude" => Agent::Claude,
        "codex" => Agent::Codex,
        "antigravity" => Agent::Antigravity,
        "gentle-shell" => Agent::GentleShell,
        "gemini" => Agent::Gemini,
        "opencode" => Agent::OpenCode,
        "cursor" => Agent::Cursor,
        _ => Agent::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use massorca_protocol::methods::agents::{ActivitySource, ActivityState, Outcome};
    use massorca_protocol::methods::terminals::TerminalState;

    use super::*;
    use crate::service::StoreUpdate;
    use crate::service::fixtures::agent;

    fn reporting(state: ActivityState, outcome: Option<Outcome>) -> Activity {
        Activity {
            state,
            outcome,
            source: ActivitySource::Hook,
            at_ms: 1,
        }
    }

    /// The status of a session whose run is `execution` and whose last report is `activity`.
    fn state_of(execution: TerminalState, activity: Option<Activity>) -> AgentState {
        let mut session = agent("s", "w1", 1);
        session.execution.state = execution;
        session.activity = activity;
        session_state(&session)
    }

    #[test]
    fn a_live_run_shows_the_state_its_agent_reported() {
        let live = |activity| state_of(TerminalState::Live, activity);
        assert_eq!(live(None), AgentState::Idle, "no report yet: not started");
        assert_eq!(
            live(Some(reporting(ActivityState::Idle, None))),
            AgentState::Idle
        );
        assert_eq!(
            live(Some(reporting(ActivityState::Working, None))),
            AgentState::Running
        );
        assert_eq!(
            live(Some(reporting(ActivityState::Waiting, None))),
            AgentState::Waiting
        );
        assert_eq!(
            live(Some(reporting(ActivityState::Done, Some(Outcome::Success)))),
            AgentState::Done
        );
        assert_eq!(
            live(Some(reporting(ActivityState::Done, Some(Outcome::Failure)))),
            AgentState::Error
        );
    }

    #[test]
    fn an_ended_run_shows_how_its_last_turn_ended() {
        let ended = |activity| state_of(TerminalState::Exited, activity);
        assert_eq!(
            ended(Some(reporting(ActivityState::Done, Some(Outcome::Success)))),
            AgentState::Done
        );
        assert_eq!(
            ended(Some(reporting(ActivityState::Done, Some(Outcome::Failure)))),
            AgentState::Error
        );
        assert_eq!(
            ended(Some(reporting(ActivityState::Working, None))),
            AgentState::Done,
            "a turn cut short by the end of the run is not still running"
        );
        assert_eq!(ended(None), AgentState::Done);
    }

    #[test]
    fn a_run_the_host_cannot_verify_is_unreachable_and_never_done() {
        for activity in [
            None,
            Some(reporting(ActivityState::Working, None)),
            Some(reporting(ActivityState::Done, Some(Outcome::Failure))),
        ] {
            assert_eq!(
                state_of(TerminalState::Unverifiable, activity),
                AgentState::Unreachable
            );
        }
    }

    #[test]
    fn each_built_in_kind_has_its_mark_and_another_kind_is_unknown() {
        assert_eq!(agent_of_kind("claude"), Agent::Claude);
        assert_eq!(agent_of_kind("codex"), Agent::Codex);
        assert_eq!(agent_of_kind("antigravity"), Agent::Antigravity);
        assert_eq!(agent_of_kind("gentle-shell"), Agent::GentleShell);
        assert_eq!(agent_of_kind("gemini"), Agent::Gemini);
        assert_eq!(agent_of_kind("opencode"), Agent::OpenCode);
        assert_eq!(agent_of_kind("cursor"), Agent::Cursor);
        assert_eq!(agent_of_kind("my-own-kind"), Agent::Unknown);
    }

    #[test]
    fn each_session_is_listed_under_its_own_workspace_oldest_first() {
        let mut second = agent("s2", "w1", 20);
        second.title = Some("Second".into());
        let mut first = agent("s1", "w1", 10);
        first.title = Some("First".into());
        let mut state = StoreState::default();
        state.apply(
            StoreUpdate::AgentsResync(vec![second, first, agent("s3", "w2", 5)]),
            Instant::now(),
        );
        let by_workspace = sessions_by_workspace(&state);
        let titles = |workspace: &str| {
            by_workspace[workspace]
                .iter()
                .map(|row| row.title.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(titles("w1"), vec!["First", "Second"]);
        assert_eq!(titles("w2"), vec![UNTITLED]);
    }
}
