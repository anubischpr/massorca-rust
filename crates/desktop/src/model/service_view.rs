//! What the shell shows for the data a service reported: the Sidebar tree, the Status bar's service
//! item and the notice above the Canvas. Plain functions over `StoreState`, so tests call them
//! without GPUI (decision 0028).

use std::path::Path;
use std::time::{Duration, Instant};

use super::agent_sessions::{agent_of_kind, session_state, top_session_rows, workspace_title};
use super::hosts::{HostTone, host_state};
use super::preflight::preflight_notice;
use super::project_icon::ProjectGlyph;
use super::shell::{
    Agent, AgentState, HostGroup, HostReach, PlaceholderRow, ProjectGroup, ServiceNotice,
    ServiceStatus, ServiceTone, ShellSnapshot, StatusBarModel, WorkspaceRow,
};
use super::teams::{TeamsState, live_teams};
use super::workspace_flow::NO_PROJECT_LABEL;
use crate::model::marketplace::DEFAULT_AGENT_KEY;
use crate::service::{ConnectionState, StoreState};
use massorca_protocol::methods::projects::{Project, ProjectKind};
use massorca_protocol::methods::workspaces::Workspace;

/// The one host a local service stands for in the tree (A10).
pub const LOCAL_HOST: &str = "This PC";
/// The usage colours cycle over workspaces; they are labels, never status (P29).
const USAGE_SLOTS: usize = 6;

/// The snapshot for a service store: projects with their workspaces under one host row, each with the
/// agent sessions it runs, and the service's own status. Usage and prototypes do not exist in the service yet.
pub fn service_snapshot(state: &StoreState, now: Instant) -> ShellSnapshot {
    let reach = reach_of(state, now);
    let listed = |workspace: &Workspace| {
        state
            .projects
            .iter()
            .any(|project| project.id == workspace.project_id)
    };
    let shown = |workspace: &Workspace| !hidden_team_folder(state, workspace);
    let shown_id = |id: &str| state.workspace(id).is_none_or(&shown);
    let mut projects: Vec<ProjectGroup> =
        state
            .projects
            .iter()
            // The Bot teams folder project shows only while one of its team folders does.
            .filter(|project| {
                !is_teams_folder(&project.path)
                    || state
                        .workspaces
                        .iter()
                        .any(|workspace| workspace.project_id == project.id && shown(workspace))
            })
            .map(|project| ProjectGroup {
                id: project.id.clone(),
                name: project.name.clone(),
                icon: ProjectGlyph::of(project.icon.as_ref()),
                owner: remote_owner_of(project),
                plain_folder: project.kind == ProjectKind::Folder,
                hosts: vec![HostGroup {
                    name: LOCAL_HOST.to_owned(),
                    reach,
                    workspaces: workspace_rows(state.workspaces.iter().filter(|workspace| {
                        workspace.project_id == project.id && shown(workspace)
                    })),
                    sessions: top_session_rows(state, |id| {
                        shown_id(id)
                            && state
                                .workspace(id)
                                .is_some_and(|workspace| workspace.project_id == project.id)
                    }),
                    placeholders: placeholder_rows(state, |workspace| {
                        workspace.project_id == project.id && shown(workspace)
                    }),
                }],
            })
            .collect();
    // A conversation's folder belongs to the hidden project of the conversations folder, which the service never
    // lists (decision 0092). Any workspace whose project is not listed is one of those: it goes under No project, last.
    let conversations = top_session_rows(state, |id| {
        state
            .workspace(id)
            .is_some_and(|workspace| !listed(workspace))
    });
    if !conversations.is_empty() || state.workspaces.iter().any(|w| !listed(w)) {
        projects.push(ProjectGroup {
            id: NO_PROJECT_LABEL.to_owned(),
            name: NO_PROJECT_LABEL.to_owned(),
            icon: ProjectGlyph::Folder,
            owner: None,
            plain_folder: false,
            hosts: vec![HostGroup {
                name: LOCAL_HOST.to_owned(),
                reach,
                workspaces: workspace_rows(state.workspaces.iter().filter(|w| !listed(w))),
                sessions: conversations,
                placeholders: placeholder_rows(state, |workspace| !listed(workspace)),
            }],
        });
    }
    ShellSnapshot {
        projects,
        usage: Vec::new(),
        status: StatusBarModel {
            waiting: count_in(state, AgentState::Waiting),
            running: count_in(state, AgentState::Running),
            // No service reports memory or open ports yet, so their items stay hidden.
            memory: None,
            terminals: super::terminals::running_count(state),
            ports: None,
            hosts_total: u32::try_from(state.hosts.len() + 1).unwrap_or(u32::MAX),
            hosts_unreachable: unreachable_hosts(state, now),
        },
        session_title: None,
        active_session: None,
        prototypes: 0,
        service_status: Some(service_status(&state.connection, now)),
        service_notice: service_notice(state),
        preflight: preflight_notice(state.connection.is_connected(), state.service_git.as_ref()),
    }
}

/// The folder under the workspaces root that holds every team's folder (`bot-teams`, as the service names it).
const TEAMS_FOLDER: &str = "bot-teams";

/// Whether `path` is the Bot teams folder project, which holds the teams' folders.
fn is_teams_folder(path: &str) -> bool {
    Path::new(path)
        .file_name()
        .is_some_and(|name| name == TEAMS_FOLDER)
}

/// A team's own folder shows in Workspaces only while that team's Workspaces option is on (AUTO-12, pv96). The
/// folder is the workspace the service makes for the team in the Bot teams folder, named with the team's id. While the
/// teams are not read, the option reads as off, its default. A bot task's worktree in a linked project stays shown.
fn hidden_team_folder(state: &StoreState, workspace: &Workspace) -> bool {
    let in_teams_folder = Path::new(&workspace.path)
        .parent()
        .is_some_and(|parent| is_teams_folder(&parent.to_string_lossy()));
    if !in_teams_folder {
        return false;
    }
    !state
        .teams
        .iter()
        .flatten()
        .any(|record| record.team.id == workspace.name && record.team.workspaces)
}

/// The rows of `workspaces` for the palette; usage colours cycle over them in order.
fn workspace_rows<'a>(workspaces: impl Iterator<Item = &'a Workspace>) -> Vec<WorkspaceRow> {
    workspaces
        .enumerate()
        .map(|(index, workspace)| WorkspaceRow {
            id: workspace.id.clone(),
            name: workspace.name.clone(),
            branch: workspace.branch.clone(),
            usage_slot: u8::try_from(index % USAGE_SLOTS + 1).unwrap_or(1),
        })
        .collect()
}

/// The workspaces `keep` accepts that have no session yet, as placeholder rows (A97). Only a workspace this app saw
/// created is listed. Its row shows the chosen agent, else the configured default, else a neutral glyph.
fn placeholder_rows(state: &StoreState, keep: impl Fn(&Workspace) -> bool) -> Vec<PlaceholderRow> {
    let default_agent = state
        .config_value(DEFAULT_AGENT_KEY)
        .and_then(|value| value.value.as_str())
        .map(agent_of_kind);
    state
        .workspaces
        .iter()
        .filter(|workspace| keep(workspace) && !state.workspace_has_session(&workspace.id))
        .filter_map(|workspace| {
            let chosen = state.awaiting_session.get(&workspace.id)?;
            Some(PlaceholderRow {
                workspace_id: workspace.id.clone(),
                title: workspace_title(workspace),
                agent: chosen
                    .as_deref()
                    .map(agent_of_kind)
                    .or(default_agent)
                    .unwrap_or(Agent::Unknown),
                branch: workspace.branch.clone(),
                path: Some(workspace.path.clone()),
            })
        })
        .collect()
}

/// How many agent sessions show `wanted`, for the Status bar's agents item.
fn count_in(state: &StoreState, wanted: AgentState) -> u32 {
    let count = state
        .agents
        .iter()
        .filter(|session| session_state(session) == wanted)
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// The hosts that cannot be reached: This PC while its link is down, and each paired host whose last
/// check found it unreachable. A host still being checked is not counted.
fn unreachable_hosts(state: &StoreState, now: Instant) -> u32 {
    let paired = state
        .hosts
        .iter()
        .filter(|entry| host_state(entry, now).tone == HostTone::Unreachable)
        .count();
    let this_pc = usize::from(!state.connection.is_connected());
    u32::try_from(paired + this_pc).unwrap_or(u32::MAX)
}

/// Connected, or unreachable for as long as the link has been down (honest state: show the age).
fn reach_of(state: &StoreState, now: Instant) -> HostReach {
    if state.connection.is_connected() {
        return HostReach::Connected;
    }
    let minutes = state.down_since.map_or(0, |since| {
        now.saturating_duration_since(since).as_secs() / 60
    });
    HostReach::Unreachable {
        // Under a minute still reads as "1 min ago", the smallest age the row can say.
        minutes: u32::try_from(minutes.max(1)).unwrap_or(u32::MAX),
    }
}

/// The Status bar's service item.
pub fn service_status(connection: &ConnectionState, now: Instant) -> ServiceStatus {
    match connection {
        ConnectionState::Connecting => ServiceStatus {
            tone: ServiceTone::Busy,
            text: "Connecting".to_owned(),
            tooltip: "Connecting to the MassOrca service".to_owned(),
        },
        ConnectionState::Connected { server, version } => ServiceStatus {
            tone: ServiceTone::Ok,
            text: "Service".to_owned(),
            tooltip: format!("Connected to {server} {version}"),
        },
        ConnectionState::Reconnecting { attempt, retry_at } => ServiceStatus {
            tone: ServiceTone::Warning,
            text: format!("Reconnecting, attempt {attempt}"),
            tooltip: format!("Next try {}", retry_in(*retry_at, now)),
        },
        ConnectionState::Unreachable { reason } => ServiceStatus {
            tone: ServiceTone::Unreachable,
            text: "Service not running".to_owned(),
            tooltip: reason.clone(),
        },
    }
}

/// The notice about the service link: a banner while it connects, the page when it is down.
/// None while connected.
pub fn service_notice(state: &StoreState) -> Option<ServiceNotice> {
    match &state.connection {
        ConnectionState::Connected { .. } => None,
        ConnectionState::Connecting => Some(ServiceNotice {
            tone: ServiceTone::Busy,
            message: "Connecting to the MassOrca service".to_owned(),
            detail: None,
            retry: false,
        }),
        ConnectionState::Reconnecting { attempt, .. } => Some(ServiceNotice {
            tone: ServiceTone::Warning,
            message: format!("Reconnecting to the MassOrca service, attempt {attempt}"),
            detail: None,
            retry: true,
        }),
        ConnectionState::Unreachable { reason } => Some(ServiceNotice {
            tone: ServiceTone::Unreachable,
            message: "The MassOrca service cannot be reached.".to_owned(),
            detail: Some(reason.clone()),
            retry: true,
        }),
    }
}

/// "in 3 s", or "now" once the time has come.
fn retry_in(retry_at: Instant, now: Instant) -> String {
    let left = retry_at.saturating_duration_since(now);
    if left.is_zero() {
        return "now".to_owned();
    }
    // Round up so the last second reads "in 1 s", never "in 0 s".
    let seconds = left.as_millis().div_ceil(1000).max(1);
    format!("in {seconds} s")
}

/// How long to wait before the notice's text is stale: one second while a timer shows.
pub const NOTICE_TICK: Duration = Duration::from_secs(1);

/// The Bot Teams of the connected service, as the Teams pages draw them. Without a connection, or before a
/// read has answered on it, there are no teams and nothing can change.
pub fn teams_view(state: &StoreState) -> TeamsState {
    match &state.teams {
        Some(records) if state.connection.is_connected() => {
            live_teams(&state.team_ids, LOCAL_HOST, records, &state.agent_kinds)
        }
        _ => TeamsState::unavailable(),
    }
}

/// The owner or org a project's remote shows after its name (A10). Nothing supplies one yet.
fn remote_owner_of(_project: &Project) -> Option<String> {
    // Project.remote_owner (core PR #332) is not in the protocol yet; None until it lands.
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::navigation::Mode;
    use crate::model::shell::AgentState;
    use crate::model::sidebar::{SidebarContent, TreeRow, sidebar_content};
    use crate::service::{
        HostEntry, HostLink, StoreUpdate,
        fixtures::{agent, project, workspace},
    };
    use massorca_client::HostRecord;
    use massorca_protocol::methods::agents::{Activity, ActivityState, Outcome};
    use massorca_protocol::methods::config::ConfigValue;

    fn connected() -> ConnectionState {
        ConnectionState::Connected {
            server: "massorca".into(),
            version: "0.1.0".into(),
        }
    }

    fn loaded(now: Instant) -> StoreState {
        let mut state = StoreState::default();
        state.apply(StoreUpdate::Connection(connected()), now);
        state.apply(
            StoreUpdate::Resync {
                projects: vec![project("p1", "alpha"), project("p2", "beta")],
                workspaces: vec![
                    workspace("w1", "p1", "one", 1),
                    workspace("w2", "p1", "two", 2),
                    workspace("w3", "p2", "three", 3),
                ],
            },
            now,
        );
        state
    }

    #[test]
    fn no_project_shows_an_owner_line_until_the_protocol_carries_one() {
        let now = Instant::now();
        let snapshot = service_snapshot(&loaded(now), now);
        assert!(
            snapshot
                .projects
                .iter()
                .all(|project| project.owner.is_none())
        );
    }

    fn setting(key: &str, value: &str) -> ConfigValue {
        ConfigValue {
            owner: "core".into(),
            key: key.into(),
            value: value.into(),
            ..Default::default()
        }
    }

    fn placeholders_of(state: &StoreState, now: Instant) -> Vec<PlaceholderRow> {
        service_snapshot(state, now).projects[0].hosts[0]
            .placeholders
            .clone()
    }

    fn tree_rows(state: &StoreState, now: Instant) -> Vec<TreeRow> {
        let snapshot = service_snapshot(state, now);
        let SidebarContent::Workspaces { tree, .. } = sidebar_content(&snapshot, Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        tree
    }

    /// Loaded p1 with w1 and w2; then w4 is created in p1 and its agent kind is chosen as `kind`.
    fn created_with_kind(now: Instant, kind: &str) -> StoreState {
        let mut state = loaded(now);
        state.apply(
            StoreUpdate::WorkspaceChanged(workspace("w4", "p1", "four", 4)),
            now,
        );
        state.apply(
            StoreUpdate::WorkspaceKindChosen {
                workspace: "w4".into(),
                kind: kind.into(),
            },
            now,
        );
        state
    }

    #[test]
    fn a_workspace_created_with_no_session_is_a_placeholder_of_its_chosen_agent() {
        let now = Instant::now();
        let mut state = loaded(now);
        assert!(
            placeholders_of(&state, now).is_empty(),
            "the workspaces listed at startup have no row"
        );
        state.apply(
            StoreUpdate::WorkspaceChanged(workspace("w4", "p1", "four", 4)),
            now,
        );
        state.apply(
            StoreUpdate::WorkspaceKindChosen {
                workspace: "w4".into(),
                kind: "codex".into(),
            },
            now,
        );
        assert_eq!(
            placeholders_of(&state, now),
            vec![PlaceholderRow {
                workspace_id: "w4".into(),
                title: "feat/four".into(),
                agent: Agent::Codex,
                branch: Some("feat/four".into()),
                path: Some("/repos/four".into()),
            }]
        );
    }

    #[test]
    fn the_first_session_takes_the_placeholders_place_in_the_tree() {
        let now = Instant::now();
        let mut state = created_with_kind(now, "codex");
        state.apply(
            StoreUpdate::AgentChanged(Box::new(agent("s0", "w1", 1))),
            now,
        );
        let placeholder = tree_rows(&state, now)
            .iter()
            .position(|row| matches!(row, TreeRow::Placeholder { .. }))
            .expect("the placeholder is listed");

        state.apply(
            StoreUpdate::AgentChanged(Box::new(agent("s4", "w4", 5))),
            now,
        );
        let rows = tree_rows(&state, now);
        assert!(
            !rows
                .iter()
                .any(|row| matches!(row, TreeRow::Placeholder { .. }))
        );
        assert!(matches!(
            &rows[placeholder],
            TreeRow::Session { id, depth: 0, .. } if id == "s4"
        ));
    }

    #[test]
    fn a_workspace_with_no_session_left_has_no_row_and_no_placeholder_comes_back() {
        let now = Instant::now();
        let mut state = created_with_kind(now, "codex");
        state.apply(
            StoreUpdate::AgentChanged(Box::new(agent("s4", "w4", 5))),
            now,
        );
        state.apply(StoreUpdate::AgentGone("s4".into()), now);
        assert!(placeholders_of(&state, now).is_empty());
        assert!(!state.awaiting_session.contains_key("w4"));
        assert!(
            tree_rows(&state, now)
                .iter()
                .all(|row| !matches!(row, TreeRow::Session { .. } | TreeRow::Placeholder { .. }))
        );
    }

    #[test]
    fn a_workspace_the_service_made_takes_the_configured_default_kind() {
        let now = Instant::now();
        let mut state = loaded(now);
        state.apply(
            StoreUpdate::ConfigResync(vec![setting(DEFAULT_AGENT_KEY, "gemini")]),
            now,
        );
        state.apply(
            StoreUpdate::WorkspaceChanged(workspace("w4", "p1", "four", 4)),
            now,
        );
        assert_eq!(placeholders_of(&state, now)[0].agent, Agent::Gemini);
    }

    #[test]
    fn a_workspace_with_no_known_kind_at_all_is_a_neutral_glyph() {
        let now = Instant::now();
        let mut state = loaded(now);
        state.apply(
            StoreUpdate::WorkspaceChanged(workspace("w4", "p1", "four", 4)),
            now,
        );
        assert_eq!(placeholders_of(&state, now)[0].agent, Agent::Unknown);
    }

    #[test]
    fn the_chosen_kind_wins_over_the_default_whichever_arrives_first() {
        let now = Instant::now();
        let mut state = loaded(now);
        state.apply(
            StoreUpdate::ConfigResync(vec![setting(DEFAULT_AGENT_KEY, "gemini")]),
            now,
        );
        state.apply(
            StoreUpdate::WorkspaceChanged(workspace("w4", "p1", "four", 4)),
            now,
        );
        state.apply(
            StoreUpdate::WorkspaceKindChosen {
                workspace: "w4".into(),
                kind: "codex".into(),
            },
            now,
        );
        assert_eq!(placeholders_of(&state, now)[0].agent, Agent::Codex);
    }

    #[test]
    fn a_workspace_announced_before_the_first_list_is_not_waiting_for_a_session() {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(StoreUpdate::Connection(connected()), now);
        state.apply(
            StoreUpdate::WorkspaceChanged(workspace("w1", "p1", "one", 1)),
            now,
        );
        state.apply(
            StoreUpdate::Resync {
                projects: vec![project("p1", "alpha")],
                workspaces: vec![workspace("w1", "p1", "one", 1)],
            },
            now,
        );
        assert!(placeholders_of(&state, now).is_empty());
    }

    #[test]
    fn the_teams_stay_unavailable_until_a_read_answers_on_a_connected_service() {
        let now = Instant::now();
        let mut state = StoreState::default();
        assert!(!teams_view(&state).is_connected());
        let connected = ConnectionState::Connected {
            server: "vps".into(),
            version: "1".into(),
        };
        state.apply(crate::service::StoreUpdate::Connection(connected), now);
        assert!(
            !teams_view(&state).is_connected(),
            "no read has answered yet"
        );
        let read = crate::service::TeamsRead {
            number: 1,
            teams: Ok(vec![crate::service::fixtures::team("t1", "Desarrollo")]),
        };
        state.apply(crate::service::StoreUpdate::Teams(read), now);
        let view = teams_view(&state);
        assert!(view.is_connected());
        assert_eq!(view.hosts()[0].name, LOCAL_HOST);
        assert_eq!(view.teams()[0].name, "Desarrollo");
        let lost = ConnectionState::Reconnecting {
            attempt: 1,
            retry_at: now,
        };
        state.apply(crate::service::StoreUpdate::Connection(lost), now);
        assert!(!teams_view(&state).is_connected());
    }

    /// A service with project alpha and the Bot teams folder project holding team t1's folder, and team t1 read with
    /// its Workspaces option set to `workspaces`.
    fn with_team_folder(workspaces: Option<bool>) -> StoreState {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(StoreUpdate::Connection(connected()), now);
        let mut teams = project("pt", "Bot teams");
        teams.path = "/ws/bot-teams".into();
        let mut folder = workspace("wt", "pt", "t1", 2);
        folder.path = "/ws/bot-teams/t1".into();
        state.apply(
            StoreUpdate::Resync {
                projects: vec![project("p1", "alpha"), teams],
                workspaces: vec![workspace("w1", "p1", "one", 1), folder],
            },
            now,
        );
        if let Some(on) = workspaces {
            let mut record = crate::service::fixtures::team("t1", "Desarrollo");
            record.team.workspaces = on;
            state.apply(
                StoreUpdate::Teams(crate::service::TeamsRead {
                    number: 1,
                    teams: Ok(vec![record]),
                }),
                now,
            );
        }
        state
    }

    fn project_names(state: &StoreState) -> Vec<String> {
        service_snapshot(state, Instant::now())
            .projects
            .into_iter()
            .map(|project| project.name)
            .collect()
    }

    #[test]
    fn a_teams_folder_shows_in_workspaces_only_while_its_workspaces_option_is_on() {
        assert_eq!(
            project_names(&with_team_folder(Some(false))),
            ["alpha"],
            "off: the team folder and its empty Bot teams project are hidden"
        );
        let on = with_team_folder(Some(true));
        assert_eq!(project_names(&on), ["Bot teams", "alpha"]);
        let snapshot = service_snapshot(&on, Instant::now());
        let folders: Vec<&str> = snapshot.projects[0].hosts[0]
            .workspaces
            .iter()
            .map(|workspace| workspace.name.as_str())
            .collect();
        assert_eq!(folders, ["t1"]);
        assert_eq!(
            project_names(&with_team_folder(None)),
            ["alpha"],
            "unread teams: the option reads as off, its default"
        );
    }

    #[test]
    fn a_workspace_named_like_a_team_outside_the_teams_folder_stays_shown() {
        let mut state = with_team_folder(Some(false));
        let mut task = workspace("wx", "p1", "t1", 3);
        task.path = "/repos/alpha-worktrees/t1".into();
        state.apply(StoreUpdate::WorkspaceChanged(task), Instant::now());
        let snapshot = service_snapshot(&state, Instant::now());
        let alpha = snapshot
            .projects
            .iter()
            .find(|project| project.name == "alpha")
            .expect("alpha is listed");
        let names: Vec<&str> = alpha.hosts[0]
            .workspaces
            .iter()
            .map(|workspace| workspace.name.as_str())
            .collect();
        assert_eq!(
            names,
            ["one", "t1"],
            "a bot task's worktree stays with its project"
        );
    }

    #[test]
    fn projects_group_their_workspaces_under_one_host() {
        let snapshot = service_snapshot(&loaded(Instant::now()), Instant::now());
        assert_eq!(snapshot.projects.len(), 2);
        let alpha = &snapshot.projects[0];
        assert_eq!(alpha.name, "alpha");
        assert_eq!(alpha.hosts.len(), 1);
        assert_eq!(alpha.hosts[0].name, "This PC");
        let names: Vec<&str> = alpha.hosts[0]
            .workspaces
            .iter()
            .map(|w| w.name.as_str())
            .collect();
        assert_eq!(names, vec!["one", "two"]);
        assert_eq!(snapshot.projects[1].hosts[0].workspaces.len(), 1);
    }

    #[test]
    fn the_sidebar_tree_has_project_and_host_rows_and_no_workspace_rows() {
        let snapshot = service_snapshot(&loaded(Instant::now()), Instant::now());
        let SidebarContent::Workspaces { tree, .. } = sidebar_content(&snapshot, Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        // 2 projects and their 2 hosts; the workspaces are not rows of the tree (A97).
        assert_eq!(tree.len(), 4);
        assert!(
            tree.iter()
                .all(|row| matches!(row, TreeRow::Project { .. } | TreeRow::Host { .. }))
        );
    }

    #[test]
    fn a_hosts_sessions_list_oldest_first_across_its_workspaces() {
        let now = Instant::now();
        let mut state = loaded(now);
        state.apply(
            StoreUpdate::AgentsResync(vec![agent("late", "w2", 30), agent("early", "w1", 10)]),
            now,
        );
        let snapshot = service_snapshot(&state, now);
        let ids: Vec<&str> = snapshot.projects[0].hosts[0]
            .sessions
            .iter()
            .map(|row| row.id.as_str())
            .collect();
        assert_eq!(ids, vec!["early", "late"]);
    }

    #[test]
    fn usage_colours_cycle_through_the_six_slots() {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(StoreUpdate::Connection(connected()), now);
        state.apply(
            StoreUpdate::Resync {
                projects: vec![project("p1", "alpha")],
                workspaces: (0..8)
                    .map(|i| workspace(&format!("w{i}"), "p1", &format!("n{i}"), i))
                    .collect(),
            },
            now,
        );
        let snapshot = service_snapshot(&state, now);
        let slots: Vec<u8> = snapshot.projects[0].hosts[0]
            .workspaces
            .iter()
            .map(|w| w.usage_slot)
            .collect();
        assert_eq!(slots, vec![1, 2, 3, 4, 5, 6, 1, 2]);
    }

    #[test]
    fn a_workspace_whose_project_is_not_listed_is_a_conversation_under_no_project() {
        let now = Instant::now();
        let mut state = loaded(now);
        state.apply(
            StoreUpdate::WorkspaceChanged(workspace("w9", "hidden", "conversation", 9)),
            now,
        );
        let snapshot = service_snapshot(&state, now);
        let last = snapshot.projects.last().expect("the No project group");
        assert_eq!(last.name, NO_PROJECT_LABEL);
        let ids: Vec<&str> = last.hosts[0]
            .workspaces
            .iter()
            .map(|row| row.id.as_str())
            .collect();
        assert_eq!(ids, ["w9"]);
        let total: usize = snapshot
            .projects
            .iter()
            .map(|p| p.hosts[0].workspaces.len())
            .sum();
        assert_eq!(total, 4);
    }

    #[test]
    fn no_project_group_is_absent_without_a_conversation() {
        let now = Instant::now();
        let snapshot = service_snapshot(&loaded(now), now);
        assert!(snapshot.projects.iter().all(|p| p.name != NO_PROJECT_LABEL));
    }

    #[test]
    fn connected_shows_a_quiet_item_and_no_notice() {
        let now = Instant::now();
        let snapshot = service_snapshot(&loaded(now), now);
        let status = snapshot.service_status.expect("a service item");
        assert_eq!(status.tone, ServiceTone::Ok);
        assert_eq!(status.tooltip, "Connected to massorca 0.1.0");
        assert_eq!(snapshot.service_notice, None);
        assert_eq!(snapshot.status.hosts_unreachable, 0);
        assert!(matches!(
            snapshot.projects[0].hosts[0].reach,
            HostReach::Connected
        ));
    }

    #[test]
    fn a_live_snapshot_reports_no_memory_or_ports() {
        let now = Instant::now();
        let status = service_snapshot(&loaded(now), now).status;
        assert_eq!(status.memory_text(), None);
        assert_eq!(status.ports_text(), None);
        assert_eq!(status.bot_teams_ports_text(), None);
    }

    fn paired(name: &str, link: HostLink) -> HostEntry {
        HostEntry {
            record: HostRecord {
                name: name.into(),
                server: format!("{name}-server"),
                addresses: vec!["100.64.0.10:7347".into()],
                scopes: vec!["read".into()],
                ..crate::service::fixtures::base_host_record()
            },
            link,
            last_connected: None,
        }
    }

    fn served() -> HostLink {
        HostLink::Connected {
            server: "vps-server".into(),
            version: "0.1.0".into(),
        }
    }

    #[test]
    fn the_hosts_item_counts_this_pc_and_each_paired_host_and_names_the_unreachable_ones() {
        let now = Instant::now();
        let mut state = loaded(now);
        state.hosts = vec![
            paired("nas", HostLink::Unreachable("timed out".into())),
            paired("vps", served()),
        ];
        let status = service_snapshot(&state, now).status;
        assert_eq!(status.hosts_total, 3);
        assert_eq!(status.hosts_unreachable, 1);
        assert_eq!(status.hosts_text(), "1 of 3 hosts unreachable");
        assert_eq!(status.bot_teams_hosts_text(), "1 host unreachable");
    }

    #[test]
    fn a_host_still_being_checked_is_not_counted_as_unreachable() {
        let now = Instant::now();
        let mut state = loaded(now);
        state.hosts = vec![paired("nas", HostLink::Checking), paired("vps", served())];
        let status = service_snapshot(&state, now).status;
        assert_eq!(status.hosts_total, 3);
        assert_eq!(status.hosts_unreachable, 0);
        assert_eq!(status.hosts_text(), "3 hosts");
    }

    #[test]
    fn a_local_link_that_is_down_counts_this_pc_as_unreachable_beside_the_paired_hosts() {
        let now = Instant::now();
        let mut state = loaded(now);
        state.apply(
            StoreUpdate::Connection(ConnectionState::Unreachable {
                reason: "The service is not running.".into(),
            }),
            now,
        );
        state.hosts = vec![paired("vps", served())];
        let status = service_snapshot(&state, now).status;
        assert_eq!(status.hosts_total, 2);
        assert_eq!(status.hosts_unreachable, 1);
        assert_eq!(status.hosts_text(), "1 of 2 hosts unreachable");
    }

    #[test]
    fn connecting_shows_a_busy_notice_without_retry() {
        let notice = service_notice(&StoreState::default()).expect("a notice");
        assert_eq!(notice.tone, ServiceTone::Busy);
        assert!(!notice.retry);
    }

    #[test]
    fn reconnecting_counts_the_attempt_and_offers_only_retry_now() {
        let now = Instant::now();
        let mut state = loaded(now);
        state.apply(
            StoreUpdate::Connection(ConnectionState::Reconnecting {
                attempt: 3,
                retry_at: now + Duration::from_millis(2300),
            }),
            now,
        );
        let notice = service_notice(&state).expect("a notice");
        assert_eq!(
            notice.message,
            "Reconnecting to the MassOrca service, attempt 3"
        );
        assert_eq!(notice.detail, None, "the banner has no Details action");
        assert!(notice.retry);
        let status = service_status(&state.connection, now);
        assert_eq!(status.text, "Reconnecting, attempt 3");
        assert_eq!(status.tooltip, "Next try in 3 s");
        let later = service_status(&state.connection, now + Duration::from_secs(5));
        assert_eq!(later.tooltip, "Next try now");
    }

    #[test]
    fn unreachable_names_the_reason_and_offers_retry_now() {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(
            StoreUpdate::Connection(ConnectionState::Unreachable {
                reason: "The service is not running.".into(),
            }),
            now,
        );
        let notice = service_notice(&state).expect("a notice");
        assert_eq!(notice.tone, ServiceTone::Unreachable);
        assert_eq!(
            notice.detail.as_deref(),
            Some("The service is not running.")
        );
        assert!(notice.retry);
        assert_eq!(
            service_status(&state.connection, now).text,
            "Service not running"
        );
        let snapshot = service_snapshot(&state, now);
        assert_eq!(snapshot.status.hosts_text(), "1 of 1 hosts unreachable");
    }

    #[test]
    fn live_sessions_sit_under_their_host_and_count_in_the_status_bar() {
        let now = Instant::now();
        let mut state = loaded(now);
        let report = |state, outcome| Activity {
            state,
            outcome,
            ..crate::service::fixtures::base_activity()
        };
        let mut working = agent("s1", "w1", 10);
        working.activity = Some(report(ActivityState::Working, None));
        let mut waiting = agent("s2", "w1", 20);
        waiting.activity = Some(report(ActivityState::Waiting, None));
        let mut done = agent("s3", "w3", 30);
        done.activity = Some(report(ActivityState::Done, Some(Outcome::Success)));
        state.apply(StoreUpdate::AgentsResync(vec![working, waiting, done]), now);

        let snapshot = service_snapshot(&state, now);
        let listed = |project: usize| {
            snapshot.projects[project].hosts[0]
                .sessions
                .iter()
                .map(|row| (row.id.as_str(), row.state))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            listed(0),
            vec![("s1", AgentState::Running), ("s2", AgentState::Waiting)]
        );
        assert_eq!(listed(1), vec![("s3", AgentState::Done)]);
        assert_eq!(snapshot.status.running, 1);
        assert_eq!(snapshot.status.waiting, 1);
    }

    #[test]
    fn the_sidebar_indents_children_and_titles_untitled_ones_by_their_branch() {
        let now = Instant::now();
        let mut state = loaded(now);
        let mut plan = agent("p", "w1", 10);
        plan.title = Some("Plan".into());
        let mut child = agent("c", "w2", 20);
        child.parent_id = Some("p".into());
        let mut check = agent("g", "w2", 30);
        check.parent_id = Some("c".into());
        check.title = Some("Check".into());
        state.apply(StoreUpdate::AgentsResync(vec![plan, child, check]), now);

        let snapshot = service_snapshot(&state, now);
        let SidebarContent::Workspaces { tree, .. } = sidebar_content(&snapshot, Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        let sessions: Vec<(&str, usize)> = tree
            .iter()
            .filter_map(|row| match row {
                TreeRow::Session { title, depth, .. } => Some((title.as_str(), *depth)),
                _ => None,
            })
            .collect();
        assert_eq!(sessions, vec![("Plan", 0), ("feat/two", 1), ("Check", 2)]);
        let child_tooltip = tree.iter().find_map(|row| match row {
            TreeRow::Session { id, tooltip, .. } if id == "c" => tooltip.clone(),
            _ => None,
        });
        assert_eq!(child_tooltip.as_deref(), Some("feat/two · /repos/two"));
    }

    #[test]
    fn a_lost_link_keeps_the_last_known_tree_with_its_age() {
        let t0 = Instant::now();
        let mut state = loaded(t0);
        state.apply(
            StoreUpdate::Connection(ConnectionState::Reconnecting {
                attempt: 1,
                retry_at: t0,
            }),
            t0,
        );
        let snapshot = service_snapshot(&state, t0 + Duration::from_secs(5 * 60));
        assert_eq!(snapshot.projects.len(), 2, "the last known state stays");
        assert!(matches!(
            snapshot.projects[0].hosts[0].reach,
            HostReach::Unreachable { minutes: 5 }
        ));
    }
}
