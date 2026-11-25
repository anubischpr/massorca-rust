//! The Sidebar's rows in each mode: the Workspaces navigation and project tree. The Bot Teams
//! rows come from the teams sidebar (D-WS-29); here Bot Teams is only the marker (A9, A10, A83).

use std::collections::BTreeSet;
use std::iter::Peekable;

use super::navigation::{Mode, Page};
use super::shell::{Agent, AgentState, HostReach, ProjectGroup, SessionRow, ShellSnapshot};
use super::workspace_flow::NO_PROJECT_LABEL;

/// The icon of a navigation row. The view maps each kind to a Lucide icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavKind {
    Prototypes,
    Tasks,
    Automations,
    Mobile,
}

/// A row above or instead of the tree: a link to a page, or a placeholder with none yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavRow {
    pub kind: NavKind,
    pub label: &'static str,
    /// A trailing count, such as the number of prototypes.
    pub count: Option<u32>,
    pub page: Option<Page>,
    /// A closed disclosure chevron at the trailing edge (Prototypes, D-WS-01).
    pub chevron: bool,
}

/// One row of the project tree, top to bottom: a project, its hosts, their workspaces and
/// their sessions.
#[derive(Clone, Debug, PartialEq)]
pub enum TreeRow {
    Project {
        name: String,
        /// The project's emoji icon, drawn before its name.
        emoji: Option<String>,
    },
    Host {
        name: String,
        reach: HostReach,
    },
    /// A paired remote host, listed after the projects with the state its last check left (D-HO-01).
    RemoteHost {
        name: String,
        state: crate::model::hosts::HostState,
    },
    Workspace {
        /// The workspace's id, for actions on the row.
        id: String,
        name: String,
        branch: Option<String>,
        usage_slot: u8,
    },
    /// A session at `depth` (0 for a top session, 1 for its child sessions, A83).
    Session {
        /// The session's id, for the actions on its row.
        id: String,
        title: String,
        agent: Agent,
        state: AgentState,
        depth: usize,
        selected: bool,
        /// Whether the row offers to bring this child back.
        can_bring_back: bool,
        /// An unread session: shown with a dot and never folded (WS-33).
        unread: bool,
        /// The row tooltip: "branch · worktree path", or the folder path (A97).
        tooltip: Option<String>,
        /// The workspace the session runs in, for the row menu.
        workspace_id: String,
        /// Its host is unreachable, so the row dims (A10).
        dimmed: bool,
    },
    /// Shows a workspace's folded sessions: `count` of its top sessions are hidden behind it.
    ShowMore {
        workspace_id: String,
        count: usize,
        /// The worst state among the hidden sessions, if the row marks it (Error, Waiting).
        worst: Option<AgentState>,
    },
    /// Folds a workspace's sessions back behind "Show N more", once they are all listed.
    ShowLess {
        workspace_id: String,
    },
}

/// The indent of a project, a host and a top session, in px from the Sidebar's edge (A97).
pub const PROJECT_INDENT: f32 = 8.0;
pub const HOST_INDENT: f32 = 24.0;
pub const SESSION_INDENT: f32 = 40.0;
/// A child session's indent: its agent mark lines up with the first letter of its parent's title (A85).
pub const CHILD_INDENT: f32 = 62.0;

/// The indent of a session `depth` levels below the top: each child step adds the same 22 px.
pub fn session_indent(depth: usize) -> f32 {
    match depth {
        0 => SESSION_INDENT,
        depth => CHILD_INDENT + (depth - 1) as f32 * (CHILD_INDENT - SESSION_INDENT),
    }
}

/// What the Sidebar lists in the current mode.
#[derive(Clone, Debug, PartialEq)]
pub enum SidebarContent {
    Workspaces {
        nav: Vec<NavRow>,
        tree: Vec<TreeRow>,
    },
    /// The teams sidebar, which the teams module builds from the Bot Teams state (D-WS-29).
    BotTeams,
}

/// The Sidebar's content for a mode, with the open session marked as selected.
pub fn sidebar_content(snapshot: &ShellSnapshot, mode: Mode) -> SidebarContent {
    match mode {
        Mode::Workspaces => SidebarContent::Workspaces {
            nav: vec![
                NavRow {
                    kind: NavKind::Prototypes,
                    label: "Prototypes",
                    count: Some(snapshot.prototypes),
                    page: None,
                    chevron: true,
                },
                NavRow {
                    kind: NavKind::Tasks,
                    label: "Tasks",
                    count: None,
                    page: Some(Page::Tasks),
                    chevron: false,
                },
                NavRow {
                    kind: NavKind::Automations,
                    label: "Automations",
                    count: None,
                    page: Some(Page::Automations),
                    chevron: false,
                },
                NavRow {
                    kind: NavKind::Mobile,
                    label: "MassOrca Mobile",
                    count: None,
                    page: None,
                    chevron: false,
                },
            ],
            tree: flatten(&snapshot.projects, snapshot.active_session.as_deref()),
        },
        Mode::BotTeams => SidebarContent::BotTeams,
    }
}

/// A project row's label: its emoji before its name, when it has one.
pub fn project_label(name: &str, emoji: Option<&str>) -> String {
    match emoji {
        Some(emoji) => format!("{emoji} {name}"),
        None => name.to_owned(),
    }
}

/// The row tooltip of a session: "branch · worktree path", the path alone for a folder (A97).
pub fn tooltip_of(session: &SessionRow) -> Option<String> {
    match (&session.branch, &session.path) {
        (Some(branch), Some(path)) => Some(format!("{branch} · {path}")),
        (Some(branch), None) => Some(branch.clone()),
        (None, Some(path)) => Some(path.clone()),
        (None, None) => None,
    }
}

fn flatten(projects: &[ProjectGroup], active: Option<&str>) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    for project in projects {
        // The "No project" group is listed only while it has sessions (A95, A97).
        let empty = project.hosts.iter().all(|host| host.sessions.is_empty());
        if project.name == NO_PROJECT_LABEL && empty {
            continue;
        }
        rows.push(TreeRow::Project {
            name: project.name.clone(),
            emoji: project.emoji.clone(),
        });
        for host in &project.hosts {
            rows.push(TreeRow::Host {
                name: host.name.clone(),
                reach: host.reach,
            });
            let dimmed = matches!(host.reach, HostReach::Unreachable { .. });
            for session in &host.sessions {
                push_session(&mut rows, session, 0, active, dimmed);
            }
        }
    }
    rows
}

fn push_session(
    rows: &mut Vec<TreeRow>,
    session: &SessionRow,
    depth: usize,
    active: Option<&str>,
    dimmed: bool,
) {
    rows.push(TreeRow::Session {
        id: session.id.clone(),
        title: session.title.clone(),
        agent: session.agent,
        state: session.state,
        depth,
        selected: active == Some(session.id.as_str()),
        unread: session.unread,
        can_bring_back: session.can_bring_back,
        tooltip: tooltip_of(session),
        workspace_id: session.workspace_id.clone(),
        dimmed,
    });
    for child in &session.children {
        push_session(rows, child, depth + 1, active, dimmed);
    }
}

/// How many top-level sessions a workspace lists before the rest fold (WS-33).
pub const SESSIONS_SHOWN: usize = 5;

/// The workspaces whose folded sessions the user has unfolded, by workspace id. Kept for the app run only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SidebarFolds {
    unfolded: BTreeSet<String>,
}

impl SidebarFolds {
    pub fn unfold(&mut self, workspace_id: &str) {
        self.unfolded.insert(workspace_id.to_owned());
    }

    pub fn fold(&mut self, workspace_id: &str) {
        self.unfolded.remove(workspace_id);
    }

    pub fn is_unfolded(&self, workspace_id: &str) -> bool {
        self.unfolded.contains(workspace_id)
    }
}

/// Folds each workspace's top sessions past the fifth. The fold row comes after the listed sessions.
pub fn fold_sessions(tree: Vec<TreeRow>, folds: &SidebarFolds) -> Vec<TreeRow> {
    let mut out = Vec::with_capacity(tree.len());
    let mut rows = tree.into_iter().peekable();
    while let Some(row) = rows.next() {
        let workspace = match &row {
            TreeRow::Workspace { id, .. } => Some(id.clone()),
            _ => None,
        };
        out.push(row);
        let Some(workspace_id) = workspace else {
            continue;
        };
        let blocks = session_blocks(&mut rows);
        out.extend(fold_blocks(&workspace_id, blocks, folds));
    }
    out
}

/// The top sessions after a workspace row, each with the child sessions below it, in order.
fn session_blocks(rows: &mut Peekable<impl Iterator<Item = TreeRow>>) -> Vec<Vec<TreeRow>> {
    let mut blocks: Vec<Vec<TreeRow>> = Vec::new();
    while let Some(row) = rows.next_if(|row| matches!(row, TreeRow::Session { .. })) {
        let top = matches!(row, TreeRow::Session { depth: 0, .. });
        match blocks.last_mut() {
            Some(block) if !top => block.push(row),
            _ => blocks.push(vec![row]),
        }
    }
    blocks
}

/// One workspace's session rows: all of them when unfolded, else the listed ones and a fold row.
fn fold_blocks(
    workspace_id: &str,
    blocks: Vec<Vec<TreeRow>>,
    folds: &SidebarFolds,
) -> Vec<TreeRow> {
    let hidden: Vec<bool> = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| index >= SESSIONS_SHOWN && !stays_listed(block))
        .collect();
    let folded = hidden.iter().filter(|&&hidden| hidden).count();
    if folded == 0 {
        return blocks.into_iter().flatten().collect();
    }
    if folds.is_unfolded(workspace_id) {
        let mut rows: Vec<TreeRow> = blocks.into_iter().flatten().collect();
        rows.push(TreeRow::ShowLess {
            workspace_id: workspace_id.to_owned(),
        });
        return rows;
    }
    let worst = worst_hidden_state(
        blocks
            .iter()
            .zip(&hidden)
            .filter(|(_, hidden)| **hidden)
            .flat_map(|(block, _)| block)
            .filter_map(|row| match row {
                TreeRow::Session { state, .. } => Some(*state),
                _ => None,
            }),
    );
    let mut rows: Vec<TreeRow> = blocks
        .into_iter()
        .zip(hidden)
        .filter(|(_, hidden)| !hidden)
        .flat_map(|(block, _)| block)
        .collect();
    rows.push(TreeRow::ShowMore {
        workspace_id: workspace_id.to_owned(),
        count: folded,
        worst,
    });
    rows
}

/// Whether a top session and its children stay listed: one is open, unread, waits for the person or failed.
fn stays_listed(block: &[TreeRow]) -> bool {
    block.iter().any(|row| {
        matches!(row, TreeRow::Session { state, selected, unread, .. } if *selected || *unread || state.never_folds())
    })
}

/// The worst state a fold row marks among the sessions it hides: Error, else Waiting. Every other
/// state marks nothing; Unreachable is a lost link, not a failure.
fn worst_hidden_state(states: impl IntoIterator<Item = AgentState>) -> Option<AgentState> {
    let states: Vec<AgentState> = states.into_iter().collect();
    [AgentState::Error, AgentState::Waiting]
        .into_iter()
        .find(|worst| states.contains(worst))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{DataSource, SampleDataSource};
    use crate::model::shell::{HostGroup, StatusBarModel};

    fn sample() -> ShellSnapshot {
        SampleDataSource.snapshot()
    }

    fn labels(rows: &[NavRow]) -> Vec<&'static str> {
        rows.iter().map(|row| row.label).collect()
    }

    #[test]
    fn workspaces_list_the_navigation_rows_with_their_pages() {
        let SidebarContent::Workspaces { nav, .. } = sidebar_content(&sample(), Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        assert_eq!(
            labels(&nav),
            vec!["Prototypes", "Tasks", "Automations", "MassOrca Mobile"]
        );
        assert_eq!(nav[0].count, Some(3));
        assert_eq!(nav[2].page, Some(Page::Automations));
        assert_eq!(nav[3].page, None);
    }

    #[test]
    fn only_prototypes_shows_a_chevron() {
        let SidebarContent::Workspaces { nav, .. } = sidebar_content(&sample(), Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        let chevrons: Vec<bool> = nav.iter().map(|row| row.chevron).collect();
        assert_eq!(chevrons, [true, false, false, false]);
    }

    #[test]
    fn the_tree_has_project_host_and_session_rows_and_no_workspace_row() {
        let SidebarContent::Workspaces { tree, .. } = sidebar_content(&sample(), Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        // 2 projects, 3 hosts and 6 sessions (three of them children of another): no workspace row.
        assert_eq!(tree.len(), 11);
        assert!(
            tree.iter()
                .all(|row| !matches!(row, TreeRow::Workspace { .. }))
        );
        assert_eq!(
            tree[0],
            TreeRow::Project {
                name: "massorca-rust".into(),
                emoji: None,
            }
        );
        assert!(matches!(
            tree[1],
            TreeRow::Host { ref name, reach: HostReach::Connected } if name == "This PC"
        ));
    }

    #[test]
    fn child_sessions_sit_one_level_deeper_than_their_parent() {
        let SidebarContent::Workspaces { tree, .. } = sidebar_content(&sample(), Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        let depth_of = |wanted: &str| {
            tree.iter().find_map(|row| match row {
                TreeRow::Session { title, depth, .. } if title == wanted => Some(*depth),
                _ => None,
            })
        };
        assert_eq!(depth_of("Implementation of master catalogs"), Some(0));
        assert_eq!(depth_of("Feed endpoints"), Some(1));
        assert_eq!(depth_of("Catalog tests"), Some(1));
    }

    #[test]
    fn only_the_open_session_is_selected() {
        let SidebarContent::Workspaces { tree, .. } = sidebar_content(&sample(), Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        let selected: Vec<&str> = tree
            .iter()
            .filter_map(|row| match row {
                TreeRow::Session {
                    title,
                    selected: true,
                    ..
                } => Some(title.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(selected, vec!["Implementation of master catalogs"]);
    }

    #[test]
    fn an_unreachable_host_keeps_how_long_it_has_been_unreachable() {
        let SidebarContent::Workspaces { tree, .. } = sidebar_content(&sample(), Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        assert!(tree.iter().any(|row| matches!(
            row,
            TreeRow::Host {
                name,
                reach: HostReach::Unreachable { minutes: 5 },
            } if name == "VPS"
        )));
    }

    #[test]
    fn bot_teams_lists_no_workspace_rows() {
        assert_eq!(
            sidebar_content(&sample(), Mode::BotTeams),
            SidebarContent::BotTeams
        );
    }

    #[test]
    fn a_project_row_takes_its_emoji_into_its_label() {
        let groups = vec![ProjectGroup {
            name: "massorca".into(),
            emoji: Some("🚀".into()),
            hosts: Vec::new(),
        }];
        assert_eq!(
            flatten(&groups, None)[0],
            TreeRow::Project {
                name: "massorca".into(),
                emoji: Some("🚀".into()),
            }
        );
        assert_eq!(project_label("massorca", Some("🚀")), "🚀 massorca");
        assert_eq!(project_label("massorca", None), "massorca");
    }

    fn sample_session(id: &str) -> SessionRow {
        SessionRow {
            id: id.into(),
            title: id.into(),
            agent: Agent::Claude,
            state: AgentState::Idle,
            workspace_id: "w".into(),
            branch: Some("feat/x".into()),
            path: Some("D:/w".into()),
            place: None,
            own_worktree: false,
            can_bring_back: false,
            unread: false,
            children: Vec::new(),
        }
    }

    fn tree_of(snapshot: &ShellSnapshot) -> Vec<TreeRow> {
        let SidebarContent::Workspaces { tree, .. } = sidebar_content(snapshot, Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        tree
    }

    fn host_with(sessions: Vec<SessionRow>, reach: HostReach) -> HostGroup {
        HostGroup {
            name: "This PC".into(),
            reach,
            workspaces: Vec::new(),
            sessions,
        }
    }

    fn snapshot_of(projects: Vec<ProjectGroup>) -> ShellSnapshot {
        ShellSnapshot {
            projects,
            usage: Vec::new(),
            status: StatusBarModel {
                waiting: 0,
                running: 0,
                memory: None,
                terminals: 0,
                ports: None,
                hosts_total: 1,
                hosts_unreachable: 0,
            },
            session_title: None,
            active_session: None,
            prototypes: 0,
            service_status: None,
            service_notice: None,
        }
    }

    #[test]
    fn a_session_row_carries_the_branch_and_path_as_its_tooltip() {
        let row = tree_of(&sample()).into_iter().find_map(|row| match row {
            TreeRow::Session {
                id,
                tooltip,
                workspace_id,
                ..
            } if id == "s2" => Some((tooltip, workspace_id)),
            _ => None,
        });
        assert_eq!(
            row,
            Some((
                Some("pr/store · D:/work/core-store".to_owned()),
                "w2".to_owned()
            ))
        );
    }

    #[test]
    fn a_folder_session_tooltip_is_the_folder_path_alone() {
        let folder = SessionRow {
            branch: None,
            path: Some("D:/notes".into()),
            ..sample_session("f1")
        };
        assert_eq!(tooltip_of(&folder), Some("D:/notes".to_owned()));
        let bare = SessionRow {
            branch: None,
            path: None,
            ..sample_session("f2")
        };
        assert_eq!(tooltip_of(&bare), None);
    }

    #[test]
    fn the_sessions_of_an_unreachable_host_dim_and_the_others_do_not() {
        let dimmed: Vec<(String, bool)> = tree_of(&sample())
            .iter()
            .filter_map(|row| match row {
                TreeRow::Session { id, dimmed, .. } => Some((id.clone(), *dimmed)),
                _ => None,
            })
            .collect();
        assert!(dimmed.contains(&("s3".to_owned(), true)));
        assert!(dimmed.contains(&("s1".to_owned(), false)));
    }

    #[test]
    fn the_no_project_group_is_hidden_while_it_has_no_sessions() {
        let empty = ProjectGroup {
            name: NO_PROJECT_LABEL.into(),
            emoji: None,
            hosts: vec![host_with(Vec::new(), HostReach::Connected)],
        };
        assert!(tree_of(&snapshot_of(vec![empty.clone()])).is_empty());

        let listed = ProjectGroup {
            hosts: vec![host_with(vec![sample_session("n1")], HostReach::Connected)],
            ..empty
        };
        let rows = tree_of(&snapshot_of(vec![listed]));
        assert_eq!(rows.len(), 3, "project, host and the session");
        assert!(matches!(&rows[0], TreeRow::Project { name, .. } if name == NO_PROJECT_LABEL));
    }

    #[test]
    fn a_stopped_session_stays_listed_however_many_sessions_there_are() {
        let sessions: Vec<SessionRow> = (1..=9).map(|n| sample_session(&format!("s{n}"))).collect();
        let snapshot = snapshot_of(vec![ProjectGroup {
            name: "p".into(),
            emoji: None,
            hosts: vec![host_with(sessions, HostReach::Connected)],
        }]);
        let listed = tree_of(&snapshot)
            .iter()
            .filter(|row| matches!(row, TreeRow::Session { .. }))
            .count();
        assert_eq!(listed, 9);
    }

    #[test]
    fn indents_are_eight_twenty_four_forty_and_sixty_two_px() {
        assert_eq!(
            [PROJECT_INDENT, HOST_INDENT, SESSION_INDENT, CHILD_INDENT],
            [8.0, 24.0, 40.0, 62.0]
        );
        assert_eq!(session_indent(0), 40.0);
        assert_eq!(session_indent(1), 62.0);
        // A grandchild steps in again by the same 22 px, so its mark stays under its parent's title.
        assert_eq!(session_indent(2), 84.0);
    }
}
