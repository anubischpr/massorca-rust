//! What the shell shows for the data a service reported: the Sidebar tree, the Status bar's service
//! item and the notice above the Canvas. Plain functions over `StoreState`, so tests call them
//! without GPUI (decision 0028).

use std::time::{Duration, Instant};

use super::shell::{
    HostGroup, HostReach, ProjectGroup, ServiceNotice, ServiceStatus, ServiceTone, ShellSnapshot,
    StatusBarModel, WorkspaceRow,
};
use super::teams::{TeamsState, live_teams};
use crate::service::{ConnectionState, StoreState};

/// The one host a local service stands for in the tree (A10).
pub const LOCAL_HOST: &str = "This PC";
/// The usage colours cycle over workspaces; they are labels, never status (P29).
const USAGE_SLOTS: usize = 6;

/// The snapshot for a service store: projects with their workspaces under one host row, and the
/// service's own status. Sessions, usage and prototypes do not exist in the service yet.
pub fn service_snapshot(state: &StoreState, now: Instant) -> ShellSnapshot {
    let reach = reach_of(state, now);
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
                    sessions: Vec::new(),
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
            waiting: 0,
            running: 0,
            memory: "-".to_owned(),
            terminals: super::terminals::running_count(state),
            ports: 0,
            hosts_total: 1,
            hosts_unreachable: u32::from(!state.connection.is_connected()),
        },
        session_title: None,
        active_session: None,
        prototypes: 0,
        service_status: Some(service_status(&state.connection, now)),
        service_notice: service_notice(state),
    }
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
            live_teams(&state.team_ids, LOCAL_HOST, records)
        }
        _ => TeamsState::unavailable(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::navigation::Mode;
    use crate::model::sidebar::{SidebarContent, TreeRow, sidebar_content};
    use crate::service::{
        StoreUpdate,
        fixtures::{project, workspace},
    };

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
