//! The Sidebar's rows in each mode: the Workspaces navigation and project tree. The tree is project, host, then
//! the host's sessions; it has no workspace row (A97). The Bot Teams rows come from the teams sidebar (D-WS-29);
//! here Bot Teams is only the marker (A9, A10, A83).

use std::collections::BTreeSet;

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

/// One row of the project tree, top to bottom: a project, its host, then that host's sessions (A97).
#[derive(Clone, Debug, PartialEq)]
pub enum TreeRow {
    Project {
        /// The project's id, for collapsing it.
        id: String,
        name: String,
        /// The project's emoji icon, drawn before its name.
        emoji: Option<String>,
        /// Whether its rows are listed below it (A10).
        expanded: bool,
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
    /// One session at `depth` (0 for a top session, 1 for its child sessions, A83).
    Session {
        /// The session's id, for the actions on its row.
        id: String,
        /// The session's title, or its branch (or folder name) until the agent sets one (A97).
        title: String,
        agent: Agent,
        state: AgentState,
        depth: usize,
        selected: bool,
        /// An unread session: shown with a dot (WS-33).
        unread: bool,
        /// Whether the row offers to bring this child back.
        can_bring_back: bool,
        /// The row tooltip: "branch · worktree path", or the folder path (A97).
        tooltip: Option<String>,
        /// The workspace the session runs in, for the row menu.
        workspace_id: String,
        /// Its host is unreachable, so the row dims (A10).
        dimmed: bool,
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
            id: project.id.clone(),
            name: project.name.clone(),
            emoji: project.emoji.clone(),
            expanded: true,
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

/// The tree with its collapsed projects folded: a collapsed project keeps its row and hides the rows
/// below it, up to the next project (A10, SidebarTree.md).
pub fn collapse_projects(tree: Vec<TreeRow>, collapsed: &BTreeSet<String>) -> Vec<TreeRow> {
    let mut out = Vec::with_capacity(tree.len());
    let mut hidden = false;
    for row in tree {
        match row {
            TreeRow::Project {
                id, name, emoji, ..
            } => {
                let expanded = !collapsed.contains(&id);
                hidden = !expanded;
                out.push(TreeRow::Project {
                    id,
                    name,
                    emoji,
                    expanded,
                });
            }
            row if !hidden => out.push(row),
            _ => {}
        }
    }
    out
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

    fn tree_of(snapshot: &ShellSnapshot) -> Vec<TreeRow> {
        let SidebarContent::Workspaces { tree, .. } = sidebar_content(snapshot, Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        tree
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
        let tree = tree_of(&sample());
        // 2 projects, 3 hosts and 6 sessions (three of them children of another): no workspace row.
        assert_eq!(tree.len(), 11);
        assert_eq!(
            tree[0],
            TreeRow::Project {
                id: "massorca-rust".into(),
                name: "massorca-rust".into(),
                emoji: None,
                expanded: true,
            }
        );
        assert!(matches!(
            tree[1],
            TreeRow::Host { ref name, reach: HostReach::Connected } if name == "This PC"
        ));
        assert!(matches!(
            tree[2],
            TreeRow::Session { ref title, depth: 0, .. }
                if title == "Implementation of master catalogs"
        ));
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
    fn child_sessions_sit_one_level_deeper_than_their_parent() {
        let tree = tree_of(&sample());
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

    #[test]
    fn only_the_open_session_is_selected() {
        let selected: Vec<String> = tree_of(&sample())
            .iter()
            .filter_map(|row| match row {
                TreeRow::Session {
                    title,
                    selected: true,
                    ..
                } => Some(title.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(selected, vec!["Implementation of master catalogs"]);
    }

    #[test]
    fn an_unreachable_host_keeps_how_long_it_has_been_unreachable() {
        assert!(tree_of(&sample()).iter().any(|row| matches!(
            row,
            TreeRow::Host {
                name,
                reach: HostReach::Unreachable { minutes: 5 },
            } if name == "VPS"
        )));
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
    fn bot_teams_lists_no_workspace_rows() {
        assert_eq!(
            sidebar_content(&sample(), Mode::BotTeams),
            SidebarContent::BotTeams
        );
    }

    #[test]
    fn a_project_row_takes_its_emoji_into_its_label() {
        let groups = vec![ProjectGroup {
            id: "massorca".into(),
            name: "massorca".into(),
            emoji: Some("🚀".into()),
            hosts: Vec::new(),
        }];
        assert_eq!(
            flatten(&groups, None)[0],
            TreeRow::Project {
                id: "massorca".into(),
                name: "massorca".into(),
                emoji: Some("🚀".into()),
                expanded: true,
            }
        );
        assert_eq!(project_label("massorca", Some("🚀")), "🚀 massorca");
        assert_eq!(project_label("massorca", None), "massorca");
    }

    #[test]
    fn the_no_project_group_is_hidden_while_it_has_no_sessions() {
        let empty = ProjectGroup {
            id: NO_PROJECT_LABEL.into(),
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
            id: "p".into(),
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
    fn a_collapsed_project_keeps_its_row_and_hides_the_rows_below_it() {
        let collapsed = BTreeSet::from(["massorca-rust".to_owned()]);
        let rows = collapse_projects(tree_of(&sample()), &collapsed);
        assert_eq!(
            rows.len(),
            4,
            "the collapsed project's row, then the other project's three rows"
        );
        assert!(matches!(
            &rows[0],
            TreeRow::Project { id, expanded: false, .. } if id == "massorca-rust"
        ));
        assert!(matches!(
            &rows[1],
            TreeRow::Project { id, expanded: true, .. } if id == "datalake"
        ));
    }
}
