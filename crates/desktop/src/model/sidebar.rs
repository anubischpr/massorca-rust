//! The Sidebar's rows in each mode: the Workspaces navigation and project tree, and the Bot
//! Teams placeholder (D-SH-01, A9, A10, A83 to A85).

use super::navigation::{Mode, Page};
use super::shell::{Agent, AgentState, HostReach, ProjectGroup, SessionRow, ShellSnapshot};

/// The icon of a navigation row. The view maps each kind to a Lucide icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavKind {
    Prototypes,
    Tasks,
    Automations,
    Mobile,
    ConferenceRoom,
    Board,
}

/// A row above or instead of the tree: a link to a page, or a placeholder with none yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavRow {
    pub kind: NavKind,
    pub label: &'static str,
    /// A trailing count, such as the number of prototypes.
    pub count: Option<u32>,
    pub page: Option<Page>,
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
    Workspace {
        name: String,
        branch: Option<String>,
        usage_slot: u8,
    },
    /// A session at `depth` (0 for a top session, 1 for its child sessions, A83).
    Session {
        title: String,
        agent: Agent,
        state: AgentState,
        depth: usize,
        selected: bool,
    },
}

/// What the Sidebar lists in the current mode.
#[derive(Clone, Debug, PartialEq)]
pub enum SidebarContent {
    Workspaces {
        nav: Vec<NavRow>,
        tree: Vec<TreeRow>,
    },
    BotTeams {
        rows: Vec<NavRow>,
    },
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
                },
                NavRow {
                    kind: NavKind::Tasks,
                    label: "Tasks",
                    count: None,
                    page: Some(Page::Tasks),
                },
                NavRow {
                    kind: NavKind::Automations,
                    label: "Automations",
                    count: None,
                    page: Some(Page::Automations),
                },
                NavRow {
                    kind: NavKind::Mobile,
                    label: "MassOrca Mobile",
                    count: None,
                    page: None,
                },
            ],
            tree: flatten(&snapshot.projects, snapshot.active_session.as_deref()),
        },
        Mode::BotTeams => SidebarContent::BotTeams {
            rows: vec![
                NavRow {
                    kind: NavKind::ConferenceRoom,
                    label: "Conference room",
                    count: None,
                    page: None,
                },
                NavRow {
                    kind: NavKind::Board,
                    label: "Board",
                    count: None,
                    page: None,
                },
            ],
        },
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
        title: session.title.clone(),
        agent: session.agent,
        state: session.state,
        depth,
        selected: active == Some(session.id.as_str()),
    });
    for child in &session.children {
        push_session(rows, child, depth + 1, active);
    }
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
    fn bot_teams_shows_the_conference_room_and_the_board() {
        let SidebarContent::BotTeams { rows } = sidebar_content(&sample(), Mode::BotTeams) else {
            panic!("bot teams content expected");
        };
        assert_eq!(labels(&rows), vec!["Conference room", "Board"]);
    }
}
