//! The Sidebar's rows in each mode: the Workspaces navigation and project tree. The Bot Teams
//! rows come from the teams sidebar (D-WS-29); here Bot Teams is only the marker (A9, A10, A83).

use std::collections::BTreeSet;
use std::iter::Peekable;

use super::navigation::{Mode, Page};
use super::shell::{Agent, AgentState, HostReach, ProjectGroup, SessionRow, ShellSnapshot};

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
        /// Where a child session runs, shown after its title.
        place: Option<String>,
        /// The child runs in another worktree than its parent's, whose row stays listed too (WS-35).
        own_worktree: bool,
        /// Whether the row offers to bring this child back.
        can_bring_back: bool,
        /// An unread session: shown with a dot and never folded (WS-33).
        unread: bool,
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

fn flatten(projects: &[ProjectGroup], active: Option<&str>) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    for project in projects {
        rows.push(TreeRow::Project {
            name: project.name.clone(),
            emoji: project.emoji.clone(),
        });
        for host in &project.hosts {
            rows.push(TreeRow::Host {
                name: host.name.clone(),
                reach: host.reach,
            });
            for workspace in &host.workspaces {
                rows.push(TreeRow::Workspace {
                    id: workspace.id.clone(),
                    name: workspace.name.clone(),
                    branch: workspace.branch.clone(),
                    usage_slot: workspace.usage_slot,
                });
                for session in &workspace.sessions {
                    push_session(&mut rows, session, 0, active);
                }
            }
        }
    }
    rows
}

fn push_session(rows: &mut Vec<TreeRow>, session: &SessionRow, depth: usize, active: Option<&str>) {
    rows.push(TreeRow::Session {
        id: session.id.clone(),
        title: session.title.clone(),
        agent: session.agent,
        state: session.state,
        depth,
        selected: active == Some(session.id.as_str()),
        place: session.place.clone(),
        own_worktree: session.own_worktree,
        can_bring_back: session.can_bring_back,
        unread: session.unread,
    });
    for child in &session.children {
        push_session(rows, child, depth + 1, active);
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
    fn the_tree_has_one_row_per_project_host_workspace_and_session() {
        let SidebarContent::Workspaces { tree, .. } = sidebar_content(&sample(), Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        // 2 projects, 3 hosts, 4 workspaces and 6 sessions (one of them a child of another).
        assert_eq!(tree.len(), 15);
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
}
