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
        /// Whether the row offers to bring this child back.
        can_bring_back: bool,
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

fn flatten(projects: &[ProjectGroup], active: Option<&str>) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    for project in projects {
        rows.push(TreeRow::Project {
            name: project.name.clone(),
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
        can_bring_back: session.can_bring_back,
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

/// Whether a top session and its children stay listed: one is open, waits for the person or failed.
fn stays_listed(block: &[TreeRow]) -> bool {
    block.iter().any(|row| {
        matches!(row, TreeRow::Session { state, selected, .. } if *selected || state.never_folds())
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
    use crate::model::shell::{HostGroup, StatusBarModel, WorkspaceRow};

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
                name: "massorca-rust".into()
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

    fn session(id: &str, state: AgentState) -> SessionRow {
        SessionRow {
            id: id.into(),
            title: id.into(),
            agent: Agent::Claude,
            state,
            place: None,
            can_bring_back: false,
            children: Vec::new(),
        }
    }

    /// Top sessions `prefix1` to `prefixN`, all idle.
    fn idle_sessions(prefix: &str, count: usize) -> Vec<SessionRow> {
        (1..=count)
            .map(|n| session(&format!("{prefix}{n}"), AgentState::Idle))
            .collect()
    }

    /// One project and host holding these workspaces, each as its id and its sessions.
    fn snapshot(workspaces: Vec<(&str, Vec<SessionRow>)>, open: Option<&str>) -> ShellSnapshot {
        let workspaces = workspaces
            .into_iter()
            .map(|(id, sessions)| WorkspaceRow {
                id: id.into(),
                name: id.into(),
                branch: None,
                usage_slot: 1,
                sessions,
            })
            .collect();
        ShellSnapshot {
            projects: vec![ProjectGroup {
                name: "massorca".into(),
                hosts: vec![HostGroup {
                    name: "This PC".into(),
                    reach: HostReach::Connected,
                    workspaces,
                }],
            }],
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
            active_session: open.map(str::to_owned),
            prototypes: 0,
            service_status: None,
            service_notice: None,
        }
    }

    /// The tree a snapshot draws, folded the way the Sidebar folds it.
    fn folded(snapshot: &ShellSnapshot, folds: &SidebarFolds) -> Vec<TreeRow> {
        let SidebarContent::Workspaces { tree, .. } = sidebar_content(snapshot, Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        fold_sessions(tree, folds)
    }

    /// The text of a tree's sessions and fold rows, in order.
    fn shown(tree: &[TreeRow]) -> Vec<String> {
        tree.iter()
            .filter_map(|row| match row {
                TreeRow::Session { title, .. } => Some(title.clone()),
                TreeRow::ShowMore { count, .. } => Some(format!("Show {count} more")),
                TreeRow::ShowLess { .. } => Some("Show less".to_owned()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_workspace_of_five_sessions_or_fewer_has_no_fold_row() {
        let folds = SidebarFolds::default();
        let five = snapshot(vec![("w", idle_sessions("s", 5))], None);
        assert_eq!(
            shown(&folded(&five, &folds)),
            ["s1", "s2", "s3", "s4", "s5"]
        );
        let none = snapshot(vec![("w", Vec::new())], None);
        assert!(shown(&folded(&none, &folds)).is_empty());
    }

    #[test]
    fn the_sixth_session_folds_behind_show_one_more() {
        let six = snapshot(vec![("w", idle_sessions("s", 6))], None);
        assert_eq!(
            shown(&folded(&six, &SidebarFolds::default())),
            ["s1", "s2", "s3", "s4", "s5", "Show 1 more"]
        );
    }

    #[test]
    fn show_n_more_counts_folded_top_sessions_and_not_their_children() {
        let mut sessions = idle_sessions("s", 9);
        sessions[5].children = vec![session("c6", AgentState::Idle)];
        let nine = snapshot(vec![("w", sessions)], None);
        assert_eq!(
            shown(&folded(&nine, &SidebarFolds::default())),
            ["s1", "s2", "s3", "s4", "s5", "Show 4 more"]
        );
    }

    #[test]
    fn waiting_and_failed_sessions_never_fold_even_beyond_five() {
        let mut sessions = idle_sessions("s", 8);
        sessions[6].state = AgentState::Waiting;
        sessions[7].state = AgentState::Error;
        let tree = snapshot(vec![("w", sessions)], None);
        assert_eq!(
            shown(&folded(&tree, &SidebarFolds::default())),
            ["s1", "s2", "s3", "s4", "s5", "s7", "s8", "Show 1 more"]
        );
    }

    #[test]
    fn a_child_that_waits_or_failed_keeps_its_top_session_listed() {
        let mut sessions = idle_sessions("s", 7);
        sessions[5].children = vec![session("c6", AgentState::Error)];
        let tree = snapshot(vec![("w", sessions)], None);
        assert_eq!(
            shown(&folded(&tree, &SidebarFolds::default())),
            ["s1", "s2", "s3", "s4", "s5", "s6", "c6", "Show 1 more"]
        );
    }

    #[test]
    fn the_open_session_and_its_top_session_never_fold() {
        let open_top = snapshot(vec![("w", idle_sessions("s", 7))], Some("s7"));
        assert_eq!(
            shown(&folded(&open_top, &SidebarFolds::default())),
            ["s1", "s2", "s3", "s4", "s5", "s7", "Show 1 more"]
        );
        let mut sessions = idle_sessions("s", 7);
        sessions[5].children = vec![session("c6", AgentState::Idle)];
        let open_child = snapshot(vec![("w", sessions)], Some("c6"));
        assert_eq!(
            shown(&folded(&open_child, &SidebarFolds::default())),
            ["s1", "s2", "s3", "s4", "s5", "s6", "c6", "Show 1 more"]
        );
    }

    #[test]
    fn unfolding_lists_every_session_and_show_less_folds_them_again() {
        let tree = snapshot(vec![("w", idle_sessions("s", 7))], None);
        let mut folds = SidebarFolds::default();
        folds.unfold("w");
        assert_eq!(
            shown(&folded(&tree, &folds)),
            ["s1", "s2", "s3", "s4", "s5", "s6", "s7", "Show less"]
        );
        folds.fold("w");
        assert_eq!(
            shown(&folded(&tree, &folds)),
            ["s1", "s2", "s3", "s4", "s5", "Show 2 more"]
        );
    }

    #[test]
    fn unfolding_one_workspace_leaves_the_others_folded() {
        let tree = snapshot(
            vec![("a", idle_sessions("a", 6)), ("b", idle_sessions("b", 6))],
            None,
        );
        let mut folds = SidebarFolds::default();
        folds.unfold("a");
        assert!(folds.is_unfolded("a") && !folds.is_unfolded("b"));
        assert_eq!(
            shown(&folded(&tree, &folds)),
            [
                "a1",
                "a2",
                "a3",
                "a4",
                "a5",
                "a6",
                "Show less",
                "b1",
                "b2",
                "b3",
                "b4",
                "b5",
                "Show 1 more",
            ]
        );
    }

    #[test]
    fn an_unfolded_workspace_with_nothing_folded_has_no_show_less_row() {
        let tree = snapshot(vec![("w", idle_sessions("s", 5))], None);
        let mut folds = SidebarFolds::default();
        folds.unfold("w");
        assert_eq!(
            shown(&folded(&tree, &folds)),
            ["s1", "s2", "s3", "s4", "s5"]
        );
    }

    #[test]
    fn only_waiting_failed_and_working_sessions_never_fold() {
        let pinned = [AgentState::Waiting, AgentState::Error, AgentState::Running];
        let folding = [AgentState::Idle, AgentState::Unreachable, AgentState::Done];
        assert!(pinned.iter().all(|state| state.never_folds()));
        assert!(folding.iter().all(|state| !state.never_folds()));
    }

    #[test]
    fn the_fold_row_marks_error_before_waiting_and_nothing_else() {
        assert_eq!(
            worst_hidden_state([AgentState::Waiting, AgentState::Error, AgentState::Idle]),
            Some(AgentState::Error)
        );
        assert_eq!(
            worst_hidden_state([AgentState::Done, AgentState::Waiting]),
            Some(AgentState::Waiting)
        );
        for rest in [
            vec![AgentState::Idle, AgentState::Done, AgentState::Unreachable],
            vec![AgentState::Running],
            Vec::new(),
        ] {
            assert_eq!(worst_hidden_state(rest.iter().copied()), None, "{rest:?}");
        }
    }

    #[test]
    fn a_fold_row_marks_nothing_for_hidden_sessions_that_are_done_idle_or_unreachable() {
        let mut sessions = idle_sessions("s", 8);
        sessions[5].state = AgentState::Done;
        sessions[6].state = AgentState::Unreachable;
        sessions[6].children = vec![session("c7", AgentState::Done)];
        let tree = snapshot(vec![("w", sessions)], None);
        assert!(
            folded(&tree, &SidebarFolds::default()).contains(&TreeRow::ShowMore {
                workspace_id: "w".into(),
                count: 3,
                worst: None,
            })
        );
    }

    #[test]
    fn a_waiting_or_failed_session_stays_listed_so_the_fold_row_never_hides_it() {
        let mut sessions = idle_sessions("s", 8);
        sessions[5].state = AgentState::Waiting;
        sessions[6].state = AgentState::Error;
        let tree = snapshot(vec![("w", sessions)], None);
        let rows = folded(&tree, &SidebarFolds::default());
        assert_eq!(
            shown(&rows),
            ["s1", "s2", "s3", "s4", "s5", "s6", "s7", "Show 1 more"]
        );
        assert!(rows.contains(&TreeRow::ShowMore {
            workspace_id: "w".into(),
            count: 1,
            worst: None,
        }));
    }
}
