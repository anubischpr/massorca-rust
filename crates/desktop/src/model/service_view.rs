//! What the shell shows for the data a service reported: the Sidebar tree, the Status bar's service
//! item and the notice above the Canvas. Plain functions over `StoreState`, so tests call them
//! without GPUI (decision 0028).

use std::time::{Duration, Instant};

use super::agent_sessions::{session_state, sessions_by_workspace};
use super::hosts::{HostTone, host_state};
use super::shell::{
    AgentState, HostGroup, HostReach, ProjectGroup, ServiceNotice, ServiceStatus, ServiceTone,
    ShellSnapshot, StatusBarModel, WorkspaceRow,
};
use super::teams::{TeamsState, live_teams};
use crate::service::{ConnectionState, StoreState};

/// The one host a local service stands for in the tree (A10).
pub const LOCAL_HOST: &str = "This PC";
/// The usage colours cycle over workspaces; they are labels, never status (P29).
const USAGE_SLOTS: usize = 6;

/// The snapshot for a service store: projects with their workspaces under one host row, each with the
/// agent sessions it runs, and the service's own status. Usage and prototypes do not exist in the service yet.
pub fn service_snapshot(state: &StoreState, now: Instant) -> ShellSnapshot {
    let reach = reach_of(state, now);
    let sessions = sessions_by_workspace(state);
    let projects = state
        .projects
        .iter()
        .map(|project| {
            let workspaces = state
                .workspaces
                .iter()
                .filter(|workspace| workspace.project_id == project.id)
                .enumerate()
                .map(|(index, workspace)| WorkspaceRow {
                    id: workspace.id.clone(),
                    name: workspace.name.clone(),
                    branch: workspace.branch.clone(),
                    usage_slot: u8::try_from(index % USAGE_SLOTS + 1).unwrap_or(1),
                    sessions: sessions.get(&workspace.id).cloned().unwrap_or_default(),
                })
                .collect();
            ProjectGroup {
                name: project.name.clone(),
                hosts: vec![HostGroup {
                    name: LOCAL_HOST.to_owned(),
                    reach,
                    workspaces,
                }],
            }
        })
        .collect();
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
    }
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
    use massorca_protocol::methods::agents::{Activity, ActivitySource, ActivityState, Outcome};

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
    fn the_sidebar_tree_keeps_its_shape_and_carries_workspace_ids() {
        let snapshot = service_snapshot(&loaded(Instant::now()), Instant::now());
        let SidebarContent::Workspaces { tree, .. } = sidebar_content(&snapshot, Mode::Workspaces)
        else {
            panic!("workspaces content expected");
        };
        // 2 projects, 2 hosts, 3 workspaces.
        assert_eq!(tree.len(), 7);
        let ids: Vec<&str> = tree
            .iter()
            .filter_map(|row| match row {
                TreeRow::Workspace { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec!["w1", "w2", "w3"]);
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
    fn a_workspace_whose_project_is_unknown_is_not_drawn() {
        let now = Instant::now();
        let mut state = loaded(now);
        state.apply(
            StoreUpdate::WorkspaceChanged(workspace("w9", "gone", "stray", 9)),
            now,
        );
        let snapshot = service_snapshot(&state, now);
        let total: usize = snapshot
            .projects
            .iter()
            .map(|p| p.hosts[0].workspaces.len())
            .sum();
        assert_eq!(total, 3);
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
                pin: "sha256:test".into(),
                device_id: "dev-1".into(),
                device_name: "laptop".into(),
                scopes: vec!["read".into()],
                paired_at_ms: 1,
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
    fn live_sessions_sit_under_their_workspace_and_count_in_the_status_bar() {
        let now = Instant::now();
        let mut state = loaded(now);
        let report = |state, outcome| Activity {
            state,
            outcome,
            source: ActivitySource::Hook,
            at_ms: 1,
        };
        let mut working = agent("s1", "w1", 10);
        working.activity = Some(report(ActivityState::Working, None));
        let mut waiting = agent("s2", "w1", 20);
        waiting.activity = Some(report(ActivityState::Waiting, None));
        let mut done = agent("s3", "w3", 30);
        done.activity = Some(report(ActivityState::Done, Some(Outcome::Success)));
        state.apply(StoreUpdate::AgentsResync(vec![working, waiting, done]), now);

        let snapshot = service_snapshot(&state, now);
        let listed = |project: usize, workspace: usize| {
            snapshot.projects[project].hosts[0].workspaces[workspace]
                .sessions
                .iter()
                .map(|row| (row.id.as_str(), row.state))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            listed(0, 0),
            vec![("s1", AgentState::Running), ("s2", AgentState::Waiting)]
        );
        assert!(listed(0, 1).is_empty(), "a workspace with no session");
        assert_eq!(listed(1, 0), vec![("s3", AgentState::Done)]);
        assert_eq!(snapshot.status.running, 1);
        assert_eq!(snapshot.status.waiting, 1);
    }

    #[test]
    fn the_sidebar_indents_children_under_their_parent_and_names_where_they_run() {
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
        let sessions: Vec<(&str, usize, Option<&str>, bool)> = tree
            .iter()
            .filter_map(|row| match row {
                TreeRow::Session {
                    title,
                    depth,
                    place,
                    own_worktree,
                    ..
                } => Some((title.as_str(), *depth, place.as_deref(), *own_worktree)),
                _ => None,
            })
            .collect();
        assert_eq!(
            sessions,
            vec![
                ("Plan", 0, None, false),
                ("Child session", 1, Some("feat/two"), true),
                ("Check", 2, Some("Current worktree"), false),
            ]
        );
        let workspaces: Vec<&str> = tree
            .iter()
            .filter_map(|row| match row {
                TreeRow::Workspace { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            workspaces.contains(&"w2"),
            "the child's own worktree row stays listed"
        );
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
