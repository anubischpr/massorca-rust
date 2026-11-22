//! What the service reported, and the reducer that folds it into one state (decision 0028).
//! The store changes only through these updates, never optimistically.

use std::collections::BTreeMap;
use std::time::Instant;

use massorca_protocol::methods::git::log::GitLog;
use massorca_protocol::methods::git::status::GitStatus;
use massorca_protocol::methods::projects::Project;
use massorca_protocol::methods::terminals::Terminal;
use massorca_protocol::methods::workspaces::Workspace;

use super::connection::ConnectionState;
use super::refusal::Refusal;

/// One change the bridge sends to the UI thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreUpdate {
    Connection(ConnectionState),
    /// Every project and workspace, after a connect or an overflow. Replaces what was held.
    Resync {
        projects: Vec<Project>,
        workspaces: Vec<Workspace>,
    },
    ProjectChanged(Project),
    ProjectGone(String),
    WorkspaceChanged(Workspace),
    WorkspaceGone(String),
    /// Every terminal the service lists, after a connect. Replaces what was held.
    TerminalsResync(Vec<Terminal>),
    /// A terminal the service created, killed or reported; keyed by its id.
    TerminalChanged(Terminal),
    TerminalGone(String),
    /// What the git watch and the git calls learned about one workspace.
    Git(GitUpdate),
}

/// One answer about a workspace's git state (decisions 0010 and 0012). Each update replaces the part
/// it names, so the latest answer wins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitUpdate {
    /// The status, or why it could not be read (`git.not_a_repository` for a folder workspace).
    Status {
        workspace: String,
        result: Result<GitStatus, Refusal>,
    },
    /// The first page of history.
    Log {
        workspace: String,
        result: Result<GitLog, Refusal>,
    },
    /// Whether working-tree edits are observed; `Some` carries why they are not.
    Watch {
        workspace: String,
        degraded: Option<String>,
    },
}

impl GitUpdate {
    pub fn workspace(&self) -> &str {
        match self {
            Self::Status { workspace, .. }
            | Self::Log { workspace, .. }
            | Self::Watch { workspace, .. } => workspace,
        }
    }
}

/// The git state the store holds for one workspace. A part is `None` until its first answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceGit {
    pub status: Option<Result<GitStatus, Refusal>>,
    pub log: Option<Result<GitLog, Refusal>>,
    /// Why working-tree edits are not observed, while they are not.
    pub degraded: Option<String>,
}

/// The projects and workspaces the service reported, and how the link to it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreState {
    pub connection: ConnectionState,
    pub projects: Vec<Project>,
    pub workspaces: Vec<Workspace>,
    /// Terminals of every workspace, oldest first (decision 0033).
    pub terminals: Vec<Terminal>,
    /// True once the first full list arrived, so an empty tree can tell "none" from "not yet".
    pub loaded: bool,
    /// When the link last stopped being connected, while it is not.
    pub down_since: Option<Instant>,
    /// The git state of each workspace the store holds, by workspace id.
    pub git: BTreeMap<String, WorkspaceGit>,
}

impl Default for StoreState {
    fn default() -> Self {
        Self {
            connection: ConnectionState::Connecting,
            projects: Vec::new(),
            workspaces: Vec::new(),
            terminals: Vec::new(),
            loaded: false,
            down_since: None,
            git: BTreeMap::new(),
        }
    }
}

impl StoreState {
    /// Folds one update in. `now` stamps the moment a connected link goes down.
    pub fn apply(&mut self, update: StoreUpdate, now: Instant) {
        match update {
            StoreUpdate::Connection(state) => {
                if state.is_connected() {
                    self.down_since = None;
                } else if self.connection.is_connected() || self.down_since.is_none() {
                    self.down_since = Some(now);
                }
                self.connection = state;
            }
            StoreUpdate::Resync {
                projects,
                workspaces,
            } => {
                self.projects = projects;
                self.workspaces = workspaces;
                self.projects.sort_by(project_order);
                self.workspaces.sort_by(workspace_order);
                self.git
                    .retain(|id, _| self.workspaces.iter().any(|w| &w.id == id));
                self.loaded = true;
            }
            StoreUpdate::ProjectChanged(project) => {
                replace_by(&mut self.projects, project, |p| p.id.clone());
                self.projects.sort_by(project_order);
            }
            StoreUpdate::ProjectGone(id) => self.projects.retain(|p| p.id != id),
            StoreUpdate::WorkspaceChanged(workspace) => {
                replace_by(&mut self.workspaces, workspace, |w| w.id.clone());
                self.workspaces.sort_by(workspace_order);
            }
            StoreUpdate::WorkspaceGone(id) => {
                self.workspaces.retain(|w| w.id != id);
                self.git.remove(&id);
            }
            StoreUpdate::TerminalsResync(mut terminals) => {
                terminals.sort_by(terminal_order);
                self.terminals = terminals;
            }
            StoreUpdate::TerminalChanged(terminal) => {
                replace_by(&mut self.terminals, terminal, |t| t.id.clone());
                self.terminals.sort_by(terminal_order);
            }
            StoreUpdate::TerminalGone(id) => self.terminals.retain(|t| t.id != id),
            StoreUpdate::Git(update) => self.apply_git(update),
        }
    }

    /// An update for a workspace the store does not hold (removed, or not listed yet) changes nothing.
    fn apply_git(&mut self, update: GitUpdate) {
        if self.workspace(update.workspace()).is_none() {
            return;
        }
        let git = self.git.entry(update.workspace().to_owned()).or_default();
        match update {
            GitUpdate::Status { result, .. } => git.status = Some(result),
            GitUpdate::Log { result, .. } => git.log = Some(result),
            GitUpdate::Watch { degraded, .. } => git.degraded = degraded,
        }
    }

    /// The git state of workspace `id`, once the store has heard of it.
    pub fn git(&self, id: &str) -> Option<&WorkspaceGit> {
        self.git.get(id)
    }

    pub fn workspace(&self, id: &str) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    pub fn project(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }
}

fn replace_by<T>(items: &mut Vec<T>, item: T, id: impl Fn(&T) -> String) {
    let wanted = id(&item);
    match items.iter().position(|existing| id(existing) == wanted) {
        Some(index) => items[index] = item,
        None => items.push(item),
    }
}

/// Projects by name, as `project.list` orders them.
fn project_order(a: &Project, b: &Project) -> std::cmp::Ordering {
    a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id))
}

/// Workspaces by creation time, then name, so a new one lands last and rows do not jump.
fn terminal_order(a: &Terminal, b: &Terminal) -> std::cmp::Ordering {
    a.created_at_ms
        .cmp(&b.created_at_ms)
        .then_with(|| a.id.cmp(&b.id))
}

fn workspace_order(a: &Workspace, b: &Workspace) -> std::cmp::Ordering {
    a.created_at_ms
        .cmp(&b.created_at_ms)
        .then_with(|| a.name.cmp(&b.name))
        .then_with(|| a.id.cmp(&b.id))
}

#[cfg(test)]
pub(crate) mod fixtures {
    use massorca_protocol::methods::projects::ProjectKind;
    use massorca_protocol::methods::workspaces::{WorkspaceKind, WorkspaceState};

    use super::*;

    pub fn project(id: &str, name: &str) -> Project {
        Project {
            id: id.into(),
            kind: ProjectKind::Git,
            path: format!("/repos/{name}"),
            name: name.into(),
            workspaces_root: format!("/repos/{name}-worktrees"),
            added_at_ms: 1,
        }
    }

    pub fn workspace(id: &str, project_id: &str, name: &str, created: i64) -> Workspace {
        Workspace {
            id: id.into(),
            project_id: project_id.into(),
            kind: WorkspaceKind::Worktree,
            name: name.into(),
            path: format!("/repos/{name}"),
            branch: Some(format!("feat/{name}")),
            base_ref: None,
            created_branch: true,
            state: WorkspaceState::Ready,
            missing: false,
            created_at_ms: created,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{project, workspace};
    use super::*;

    fn resynced() -> StoreState {
        let mut state = StoreState::default();
        state.apply(
            StoreUpdate::Resync {
                projects: vec![project("p2", "zeta"), project("p1", "alpha")],
                workspaces: vec![
                    workspace("w2", "p1", "later", 20),
                    workspace("w1", "p1", "first", 10),
                ],
            },
            Instant::now(),
        );
        state
    }

    #[test]
    fn a_resync_replaces_everything_and_orders_it() {
        let state = resynced();
        let names: Vec<&str> = state.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
        let ids: Vec<&str> = state.workspaces.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, vec!["w1", "w2"]);
        assert!(state.loaded);
    }

    #[test]
    fn a_second_resync_drops_what_the_service_no_longer_has() {
        let mut state = resynced();
        state.apply(
            StoreUpdate::Resync {
                projects: vec![project("p1", "alpha")],
                workspaces: Vec::new(),
            },
            Instant::now(),
        );
        assert_eq!(state.projects.len(), 1);
        assert!(state.workspaces.is_empty());
    }

    #[test]
    fn a_changed_workspace_replaces_the_row_with_the_same_id() {
        let mut state = resynced();
        let mut renamed = workspace("w1", "p1", "first", 10);
        renamed.missing = true;
        state.apply(StoreUpdate::WorkspaceChanged(renamed), Instant::now());
        assert_eq!(state.workspaces.len(), 2);
        assert!(state.workspace("w1").is_some_and(|w| w.missing));
    }

    #[test]
    fn a_new_workspace_lands_after_older_ones() {
        let mut state = resynced();
        state.apply(
            StoreUpdate::WorkspaceChanged(workspace("w3", "p1", "newest", 30)),
            Instant::now(),
        );
        assert_eq!(state.workspaces.last().map(|w| w.id.as_str()), Some("w3"));
    }

    #[test]
    fn a_gone_workspace_or_project_is_dropped_and_an_unknown_id_is_ignored() {
        let mut state = resynced();
        state.apply(StoreUpdate::WorkspaceGone("w1".into()), Instant::now());
        state.apply(StoreUpdate::WorkspaceGone("nope".into()), Instant::now());
        state.apply(StoreUpdate::ProjectGone("p2".into()), Instant::now());
        assert_eq!(state.workspaces.len(), 1);
        assert_eq!(state.projects.len(), 1);
    }

    #[test]
    fn an_added_project_lands_in_name_order() {
        let mut state = resynced();
        state.apply(
            StoreUpdate::ProjectChanged(project("p3", "middle")),
            Instant::now(),
        );
        let names: Vec<&str> = state.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "middle", "zeta"]);
    }

    #[test]
    fn the_time_the_link_went_down_is_kept_until_it_is_back() {
        let mut state = StoreState::default();
        let up = ConnectionState::Connected {
            server: "s".into(),
            version: "1".into(),
        };
        let t0 = Instant::now();
        state.apply(StoreUpdate::Connection(up.clone()), t0);
        assert_eq!(state.down_since, None);
        let down = ConnectionState::Reconnecting {
            attempt: 1,
            retry_at: t0,
        };
        state.apply(StoreUpdate::Connection(down.clone()), t0);
        let later = t0 + std::time::Duration::from_secs(3);
        state.apply(StoreUpdate::Connection(down), later);
        assert_eq!(
            state.down_since,
            Some(t0),
            "a second down state keeps the first stamp"
        );
        state.apply(StoreUpdate::Connection(up), later);
        assert_eq!(state.down_since, None);
    }
}
