//! What the Sidebar shows for the agent sessions the service reported: each session's status, read from
//! its run and from the activity its agent reported last, and where it is listed. A child sits under its
//! parent run, wherever it runs (A83). Plain functions over the store, so tests call them without GPUI
//! (decision 0028).

use std::collections::{BTreeMap, HashMap, HashSet};

use massorca_protocol::codes;
use massorca_protocol::methods::agents::{Activity, ActivityState, AgentSession, Outcome};
use massorca_protocol::methods::terminals::TerminalState;

use super::shell::{Agent, AgentState, SessionRow};
use crate::service::{Refusal, StoreState};

/// The title of a top session the service has no title for.
const UNTITLED: &str = "Untitled session";
/// The title of a child session the service has no title for, such as a delegated one.
const CHILD_UNTITLED: &str = "Child session";
/// Where a child runs when it shares its parent's worktree.
const CURRENT_WORKTREE: &str = "Current worktree";
/// Where a child runs in its own worktree whose branch is not known.
const OWN_WORKTREE: &str = "Own worktree";

/// The sessions of each workspace as rows, oldest first. A top session is listed under its own workspace; a
/// child hangs under its parent's row. A session whose parent is not listed is a top session of its own
/// workspace, and sessions that name each other as parents hang from nothing, so they are not listed.
pub fn sessions_by_workspace(state: &StoreState) -> BTreeMap<String, Vec<SessionRow>> {
    let listed: HashSet<&str> = state.agents.iter().map(|s| s.id.as_str()).collect();
    let mut children: HashMap<&str, Vec<&AgentSession>> = HashMap::new();
    let mut tops: Vec<&AgentSession> = Vec::new();
    for session in &state.agents {
        match session.parent_id.as_deref() {
            Some(parent) if listed.contains(parent) => {
                children.entry(parent).or_default().push(session);
            }
            _ => tops.push(session),
        }
    }
    let mut by_workspace: BTreeMap<String, Vec<SessionRow>> = BTreeMap::new();
    for session in tops {
        by_workspace
            .entry(session.workspace_id.clone())
            .or_default()
            .push(session_row(session, None, &children, state));
    }
    by_workspace
}

/// A session as a row, with its place when `parent` is given and its children below it, oldest first. Each
/// session hangs from exactly one parent, so the recursion ends at the top sessions' descendants.
fn session_row(
    session: &AgentSession,
    parent: Option<&AgentSession>,
    children: &HashMap<&str, Vec<&AgentSession>>,
    state: &StoreState,
) -> SessionRow {
    SessionRow {
        id: session.id.clone(),
        title: title_of(session),
        agent: agent_of_kind(&session.kind),
        state: session_state(session),
        place: parent.map(|parent| place_of(session, parent, state)),
        children: children
            .get(session.id.as_str())
            .into_iter()
            .flatten()
            .map(|child| session_row(child, Some(session), children, state))
            .collect(),
    }
}

/// The title a row shows: the service's, or a name that says what kind of session it is.
fn title_of(session: &AgentSession) -> String {
    match (&session.title, session.parent_id.is_some()) {
        (Some(title), _) => title.clone(),
        (None, true) => CHILD_UNTITLED.to_owned(),
        (None, false) => UNTITLED.to_owned(),
    }
}

/// Where a child runs: its parent's worktree, or its own worktree by the branch it is on (DelegationTree).
fn place_of(child: &AgentSession, parent: &AgentSession, state: &StoreState) -> String {
    if child.workspace_id == parent.workspace_id {
        return CURRENT_WORKTREE.to_owned();
    }
    state
        .workspace(&child.workspace_id)
        .and_then(|workspace| workspace.branch.clone())
        .unwrap_or_else(|| OWN_WORKTREE.to_owned())
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

/// What a refused bring-back says, in plain words. A code this build does not know keeps the service's text.
pub fn bring_back_refusal(refusal: &Refusal) -> String {
    let sentence = match refusal.code() {
        Some(codes::AGENT_NOT_FOUND) => "This session no longer exists.",
        Some(codes::AGENT_ALREADY_BROUGHT_BACK) => "This session was already brought back.",
        Some(codes::AGENT_DELEGATE_NEEDS_GIT) => "The child's workspace has no branch to merge.",
        Some(codes::AGENT_PARENT_BUSY) => {
            "The parent session is working. Bring the child back once it is not."
        }
        Some(codes::AGENT_PARENT_DIRTY) => {
            "The parent workspace has uncommitted changes, or a git operation in progress. Commit, discard or finish them first."
        }
        Some(codes::AGENT_MERGE_CONFLICT) => {
            "The merge stopped on conflicts, so it was aborted and nothing changed."
        }
        Some(codes::AGENT_WORKSPACE_BUSY) => {
            "The parent workspace is being written by another session. Bring the child back once that session has stopped."
        }
        Some(codes::AGENT_BUSY) => {
            "Another bring-back is merging into this workspace. Try again once it ends."
        }
        Some(codes::AGENT_DEPTH_EXCEEDED) => "A delegated session cannot delegate.",
        _ => return refusal.message(),
    };
    sentence.to_owned()
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
    use crate::service::fixtures::{agent, project, workspace};

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

    /// A session that `parent` started, in workspace `workspace_id`.
    fn child_of(parent: &str, id: &str, workspace_id: &str, created: i64) -> AgentSession {
        let mut session = agent(id, workspace_id, created);
        session.parent_id = Some(parent.into());
        session
    }

    /// A store with two workspaces: `w1` ("one", on `feat/one`) and `w2` ("two", on `feat/two`).
    fn store_with(sessions: Vec<AgentSession>) -> StoreState {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(
            StoreUpdate::Resync {
                projects: vec![project("p1", "alpha")],
                workspaces: vec![
                    workspace("w1", "p1", "one", 1),
                    workspace("w2", "p1", "two", 2),
                ],
            },
            now,
        );
        state.apply(StoreUpdate::AgentsResync(sessions), now);
        state
    }

    #[test]
    fn a_child_nests_under_its_parent_run_in_creation_order_and_names_where_it_runs() {
        let mut parent = agent("p", "w1", 10);
        parent.title = Some("Plan".into());
        let state = store_with(vec![
            child_of("p", "c2", "w1", 30),
            child_of("p", "c1", "w2", 20),
            parent,
        ]);
        let by_workspace = sessions_by_workspace(&state);
        let top = &by_workspace["w1"];
        assert_eq!(top.len(), 1, "a child is nested, not listed on its own");
        assert_eq!(top[0].place, None, "a top session names no place");
        let children: Vec<(&str, Option<&str>)> = top[0]
            .children
            .iter()
            .map(|child| (child.id.as_str(), child.place.as_deref()))
            .collect();
        assert_eq!(
            children,
            vec![("c1", Some("feat/two")), ("c2", Some("Current worktree"))]
        );
        assert!(
            !by_workspace.contains_key("w2"),
            "the child's own workspace lists no top session"
        );
    }

    #[test]
    fn a_grandchild_nests_one_level_deeper_and_an_untitled_child_says_what_it_is() {
        let mut parent = agent("p", "w1", 10);
        parent.title = Some("Plan".into());
        let state = store_with(vec![
            parent,
            child_of("p", "c", "w2", 20),
            child_of("c", "g", "w2", 30),
        ]);
        let by_workspace = sessions_by_workspace(&state);
        let child = &by_workspace["w1"][0].children[0];
        assert_eq!(child.title, "Child session");
        let grandchild = &child.children[0];
        assert_eq!(grandchild.id, "g");
        assert_eq!(grandchild.title, "Child session");
        assert_eq!(
            grandchild.place.as_deref(),
            Some("Current worktree"),
            "the grandchild runs in its parent's worktree"
        );
    }

    #[test]
    fn a_child_whose_parent_is_gone_is_listed_as_a_top_session_of_its_own_workspace() {
        let state = store_with(vec![child_of("forgotten", "c", "w2", 20)]);
        let by_workspace = sessions_by_workspace(&state);
        let rows = &by_workspace["w2"];
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "c");
        assert_eq!(rows[0].place, None);
        assert_eq!(rows[0].title, "Child session");
    }

    #[test]
    fn sessions_that_name_each_other_as_parents_are_not_listed_and_folding_ends() {
        let state = store_with(vec![
            child_of("b", "a", "w1", 10),
            child_of("a", "b", "w1", 20),
            agent("root", "w1", 30),
        ]);
        let by_workspace = sessions_by_workspace(&state);
        let ids: Vec<&str> = by_workspace["w1"]
            .iter()
            .map(|row| row.id.as_str())
            .collect();
        assert_eq!(ids, vec!["root"], "a loop has no top session to hang from");
    }

    #[test]
    fn each_bring_back_refusal_says_why_in_plain_words() {
        let refused = |code: &str| Refusal::Failed {
            code: code.to_owned(),
            message: "the service's own text".to_owned(),
        };
        let cases = [
            (codes::AGENT_NOT_FOUND, "This session no longer exists."),
            (
                codes::AGENT_ALREADY_BROUGHT_BACK,
                "This session was already brought back.",
            ),
            (
                codes::AGENT_DELEGATE_NEEDS_GIT,
                "The child's workspace has no branch to merge.",
            ),
            (
                codes::AGENT_PARENT_BUSY,
                "The parent session is working. Bring the child back once it is not.",
            ),
            (
                codes::AGENT_PARENT_DIRTY,
                "The parent workspace has uncommitted changes, or a git operation in progress. Commit, discard or finish them first.",
            ),
            (
                codes::AGENT_MERGE_CONFLICT,
                "The merge stopped on conflicts, so it was aborted and nothing changed.",
            ),
            (
                codes::AGENT_WORKSPACE_BUSY,
                "The parent workspace is being written by another session. Bring the child back once that session has stopped.",
            ),
            (
                codes::AGENT_BUSY,
                "Another bring-back is merging into this workspace. Try again once it ends.",
            ),
            (
                codes::AGENT_DEPTH_EXCEEDED,
                "A delegated session cannot delegate.",
            ),
        ];
        for (code, sentence) in cases {
            assert_eq!(bring_back_refusal(&refused(code)), sentence, "{code}");
        }
    }

    #[test]
    fn a_refusal_without_a_plain_sentence_keeps_the_service_text() {
        let unknown = Refusal::Failed {
            code: "git.failed".to_owned(),
            message: "git said no".to_owned(),
        };
        assert_eq!(bring_back_refusal(&unknown), "git said no");
        assert_eq!(
            bring_back_refusal(&Refusal::Dirty),
            "This workspace has uncommitted changes."
        );
    }

    #[test]
    fn a_child_in_a_workspace_the_store_does_not_know_says_it_has_its_own_worktree() {
        let state = store_with(vec![agent("p", "w1", 10), child_of("p", "c", "w9", 20)]);
        let by_workspace = sessions_by_workspace(&state);
        assert_eq!(
            by_workspace["w1"][0].children[0].place.as_deref(),
            Some("Own worktree")
        );
    }
}
