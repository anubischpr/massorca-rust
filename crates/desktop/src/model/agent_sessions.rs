//! What the Sidebar shows for the agent sessions the service reported: each session's status, read from
//! its run and from the activity its agent reported last, and where it is listed. A child sits under its
//! parent run, wherever it runs (A83). Plain functions over the store, so tests call them without GPUI
//! (decision 0028).

use std::collections::{BTreeMap, HashMap, HashSet};

use massorca_protocol::codes;
use massorca_protocol::methods::agents::{Activity, ActivityState, AgentSession, Outcome};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::methods::workspaces::Workspace;

use super::shell::{Agent, AgentState, SessionRow};
use crate::service::{Refusal, StoreState};

/// The title of a session the service has no title for and whose workspace is not known.
const UNTITLED: &str = "Untitled session";
/// Where a child runs when it shares its parent's worktree.
const CURRENT_WORKTREE: &str = "Current worktree";
/// Where a child runs in its own worktree whose branch is not known.
const OWN_WORKTREE: &str = "Own worktree";

/// The sessions of each workspace as rows, oldest first. A top session is listed under its own workspace; a
/// child hangs under its parent's row. A session whose parent is not listed is a top session of its own
/// workspace, and sessions that name each other as parents hang from nothing, so they are not listed.
pub fn sessions_by_workspace(state: &StoreState) -> BTreeMap<String, Vec<SessionRow>> {
    let mut by_workspace: BTreeMap<String, Vec<SessionRow>> = BTreeMap::new();
    for row in top_session_rows(state, |_| true) {
        by_workspace
            .entry(row.workspace_id.clone())
            .or_default()
            .push(row);
    }
    by_workspace
}

/// The top sessions of the workspaces `keep` accepts, oldest first, each with its children below it (A97).
/// Sessions are listed by launch time, so a row never jumps while its agent works.
pub fn top_session_rows(state: &StoreState, keep: impl Fn(&str) -> bool) -> Vec<SessionRow> {
    rows_of(
        &state.agents,
        Listed {
            workspaces: &state.workspaces,
            seen_offered: state.agent_seen,
        },
        keep,
    )
}

/// The sessions a paired host reported as rows: the same tree as This PC's, with no unread dots (the host's
/// seen state is not read over a check) and each session's workspace from the host's own list.
pub fn host_session_rows(sessions: &[AgentSession], workspaces: &[Workspace]) -> Vec<SessionRow> {
    rows_of(
        sessions,
        Listed {
            workspaces,
            seen_offered: false,
        },
        |_| true,
    )
}

/// The sessions the rows are built from, and the workspaces they run in.
#[derive(Clone, Copy)]
struct Listed<'a> {
    workspaces: &'a [Workspace],
    seen_offered: bool,
}

impl Listed<'_> {
    fn workspace(&self, id: &str) -> Option<&Workspace> {
        self.workspaces.iter().find(|workspace| workspace.id == id)
    }
}

fn rows_of(
    sessions: &[AgentSession],
    listed: Listed<'_>,
    keep: impl Fn(&str) -> bool,
) -> Vec<SessionRow> {
    let mut ordered: Vec<&AgentSession> = sessions.iter().collect();
    ordered.sort_by_key(|session| session.created_at_ms);
    let known: HashSet<&str> = ordered.iter().map(|s| s.id.as_str()).collect();
    let mut children: HashMap<&str, Vec<&AgentSession>> = HashMap::new();
    let mut tops: Vec<&AgentSession> = Vec::new();
    for session in ordered {
        match session.parent_id.as_deref() {
            Some(parent) if known.contains(parent) => {
                children.entry(parent).or_default().push(session);
            }
            _ => tops.push(session),
        }
    }
    tops.into_iter()
        .filter(|session| keep(&session.workspace_id))
        .map(|session| session_row(session, None, &children, listed))
        .collect()
}

/// A session as a row, with its place when `parent` is given and its children below it, oldest first. Each
/// session hangs from exactly one parent, so the recursion ends at the top sessions' descendants.
fn session_row(
    session: &AgentSession,
    parent: Option<&AgentSession>,
    children: &HashMap<&str, Vec<&AgentSession>>,
    listed: Listed<'_>,
) -> SessionRow {
    let status = session_state(session);
    let workspace = listed.workspace(&session.workspace_id);
    SessionRow {
        id: session.id.clone(),
        title: title_of(session, workspace),
        agent: agent_of_kind(&session.kind),
        state: status,
        workspace_id: session.workspace_id.clone(),
        branch: workspace.and_then(|workspace| workspace.branch.clone()),
        path: workspace.map(|workspace| workspace.path.clone()),
        place: parent.map(|parent| place_of(session, parent, listed)),
        own_worktree: parent.is_some_and(|parent| parent.workspace_id != session.workspace_id),
        // Only a finished child under a parent it can merge into, and only until it is brought back.
        can_bring_back: parent.is_some()
            && session.brought_back_at_ms.is_none()
            && matches!(status, AgentState::Done | AgentState::Error),
        unread: is_unread(session, listed.seen_offered),
        children: children
            .get(session.id.as_str())
            .into_iter()
            .flatten()
            .map(|child| session_row(child, Some(session), children, listed))
            .collect(),
    }
}

/// Whether a session is unread: the service offers seen state (`agent-seen/1`) and the agent's latest `waiting` or
/// `done` has no `seen_at_ms`. The service clears `seen_at_ms` when a newer activity arrives, so a fresh report is
/// unread again. Without the capability nothing is unread.
pub fn is_unread(session: &AgentSession, seen_offered: bool) -> bool {
    seen_offered
        && session.activity.as_ref().is_some_and(|activity| {
            matches!(activity.state, ActivityState::Waiting | ActivityState::Done)
                && activity.seen_at_ms.is_none()
        })
}

/// The title a row shows: the service's, else the branch of its worktree, else the folder's name (A97).
fn title_of(session: &AgentSession, workspace: Option<&Workspace>) -> String {
    session
        .title
        .clone()
        .or_else(|| workspace.map(workspace_title))
        .unwrap_or_else(|| UNTITLED.to_owned())
}

/// A workspace's name in a row until its agent sets a title: its branch, or the folder's name for a plain folder (A97).
pub fn workspace_title(workspace: &Workspace) -> String {
    workspace
        .branch
        .clone()
        .unwrap_or_else(|| workspace.name.clone())
}

/// Where a child runs: its parent's worktree, or its own worktree by the branch it is on (DelegationTree).
fn place_of(child: &AgentSession, parent: &AgentSession, listed: Listed<'_>) -> String {
    if child.workspace_id == parent.workspace_id {
        return CURRENT_WORKTREE.to_owned();
    }
    listed
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

/// The confirm before a bring-back, in the words of the bring-back card (X-CH-08).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BringBackPrompt {
    /// The child session to bring back.
    pub child: String,
    pub title: String,
    pub body: String,
}

/// The confirm for bringing `child` back, or None when the store does not hold it under a parent it holds.
pub fn bring_back_prompt(state: &StoreState, child: &str) -> Option<BringBackPrompt> {
    let session = state.agents.iter().find(|session| session.id == child)?;
    let parent_id = session.parent_id.as_deref()?;
    let parent = state
        .agents
        .iter()
        .find(|session| session.id == parent_id)?;
    let title = match &session.title {
        Some(title) => format!("Bring back {title}?"),
        None => "Bring back this session?".to_owned(),
    };
    let body = if session.workspace_id == parent.workspace_id {
        "It ran in this worktree, so there is nothing to merge. Nothing is pushed.".to_owned()
    } else {
        let into = state.workspace(&parent.workspace_id).map_or_else(
            || "the parent workspace".to_owned(),
            |workspace| {
                workspace
                    .branch
                    .clone()
                    .unwrap_or_else(|| workspace.name.clone())
            },
        );
        format!("Merges into {into} without fast-forward. Nothing is pushed.")
    };
    Some(BringBackPrompt {
        child: child.to_owned(),
        title,
        body,
    })
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

    fn session_with(activity: Activity) -> AgentSession {
        let mut session = agent("s", "w1", 1);
        session.activity = Some(activity);
        session
    }

    #[test]
    fn a_waiting_or_done_report_without_a_seen_time_is_unread_only_when_the_service_offers_seen_state()
     {
        for state in [ActivityState::Waiting, ActivityState::Done] {
            let session = session_with(reporting(state, None));
            assert!(is_unread(&session, true), "{state:?}");
            assert!(
                !is_unread(&session, false),
                "{state:?} without agent-seen/1"
            );
        }
    }

    #[test]
    fn a_seen_report_and_the_other_states_are_not_unread() {
        let mut seen = reporting(ActivityState::Done, Some(Outcome::Success));
        seen.seen_at_ms = Some(9);
        assert!(!is_unread(&session_with(seen), true));
        for state in [
            ActivityState::Working,
            ActivityState::Idle,
            ActivityState::Unknown,
        ] {
            assert!(
                !is_unread(&session_with(reporting(state, None)), true),
                "{state:?}"
            );
        }
        assert!(!is_unread(&agent("s", "w1", 1), true), "no report yet");
    }

    #[test]
    fn the_store_marks_a_row_unread_only_with_the_capability() {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(
            StoreUpdate::AgentsResync(vec![session_with(reporting(ActivityState::Done, None))]),
            now,
        );
        let unread = |state: &StoreState| sessions_by_workspace(state)["w1"][0].unread;
        assert!(!unread(&state), "capability unknown");
        state.apply(StoreUpdate::AgentSeenOffered(true), now);
        assert!(unread(&state));
        state.apply(StoreUpdate::AgentSeenOffered(false), now);
        assert!(!unread(&state));
    }

    fn reporting(state: ActivityState, outcome: Option<Outcome>) -> Activity {
        Activity {
            state,
            outcome,
            source: ActivitySource::Hook,
            at_ms: 1,
            seen_at_ms: None,
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
    fn a_grandchild_nests_one_level_deeper_and_an_untitled_child_takes_its_branch() {
        let mut parent = agent("p", "w1", 10);
        parent.title = Some("Plan".into());
        let state = store_with(vec![
            parent,
            child_of("p", "c", "w2", 20),
            child_of("c", "g", "w2", 30),
        ]);
        let by_workspace = sessions_by_workspace(&state);
        let child = &by_workspace["w1"][0].children[0];
        assert_eq!(child.title, "feat/two");
        let grandchild = &child.children[0];
        assert_eq!(grandchild.id, "g");
        assert_eq!(grandchild.title, "feat/two");
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
        assert_eq!(rows[0].title, "feat/two");
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

    /// Whether the child `child` (under `p` in `w1`, or as given) offers a bring-back.
    fn offers_bring_back(child: AgentSession) -> bool {
        let state = store_with(vec![agent("p", "w1", 5), child]);
        sessions_by_workspace(&state)["w1"][0].children[0].can_bring_back
    }

    #[test]
    fn a_finished_child_can_be_brought_back_until_it_has_been() {
        let child = |activity: Option<Activity>, brought_back_at_ms: Option<i64>| {
            let mut child = child_of("p", "c", "w2", 20);
            child.activity = activity;
            child.brought_back_at_ms = brought_back_at_ms;
            child
        };
        let done = Some(reporting(ActivityState::Done, Some(Outcome::Success)));
        let failed = Some(reporting(ActivityState::Done, Some(Outcome::Failure)));
        assert!(offers_bring_back(child(done.clone(), None)), "finished");
        assert!(
            offers_bring_back(child(failed, None)),
            "failed turns can be merged too"
        );
        assert!(
            !offers_bring_back(child(Some(reporting(ActivityState::Working, None)), None)),
            "still working: not finished yet"
        );
        assert!(
            !offers_bring_back(child(done, Some(30))),
            "already brought back"
        );
    }

    #[test]
    fn a_top_session_or_a_child_whose_parent_is_gone_is_never_offered_a_bring_back() {
        let mut top = agent("p", "w1", 5);
        top.activity = Some(reporting(ActivityState::Done, Some(Outcome::Success)));
        let state = store_with(vec![top]);
        assert!(
            !sessions_by_workspace(&state)["w1"][0].can_bring_back,
            "a top session has no parent to merge into"
        );
        let mut orphan = child_of("gone", "c", "w2", 20);
        orphan.activity = Some(reporting(ActivityState::Done, Some(Outcome::Success)));
        let state = store_with(vec![orphan]);
        assert!(
            !sessions_by_workspace(&state)["w2"][0].can_bring_back,
            "its parent is gone"
        );
    }

    #[test]
    fn the_bring_back_confirm_names_the_branch_it_merges_into() {
        let mut child = child_of("p", "c", "w2", 20);
        child.title = Some("Feed endpoints".into());
        let state = store_with(vec![agent("p", "w1", 5), child]);
        let prompt = bring_back_prompt(&state, "c").expect("a child under a held parent");
        assert_eq!(prompt.child, "c");
        assert_eq!(prompt.title, "Bring back Feed endpoints?");
        assert_eq!(
            prompt.body,
            "Merges into feat/one without fast-forward. Nothing is pushed."
        );
    }

    #[test]
    fn a_child_in_its_parents_worktree_says_there_is_nothing_to_merge() {
        let state = store_with(vec![agent("p", "w1", 5), child_of("p", "c", "w1", 20)]);
        let prompt = bring_back_prompt(&state, "c").expect("a child under a held parent");
        assert_eq!(prompt.title, "Bring back this session?");
        assert_eq!(
            prompt.body,
            "It ran in this worktree, so there is nothing to merge. Nothing is pushed."
        );
    }

    #[test]
    fn there_is_no_bring_back_confirm_without_a_parent_in_the_store() {
        let state = store_with(vec![agent("p", "w1", 5), child_of("p", "c", "w2", 20)]);
        assert_eq!(bring_back_prompt(&state, "missing"), None);
        assert_eq!(
            bring_back_prompt(&state, "p"),
            None,
            "a top session has nothing to merge into"
        );
        let orphans = store_with(vec![child_of("gone", "o", "w2", 20)]);
        assert_eq!(bring_back_prompt(&orphans, "o"), None);
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
