//! Where the shell's view models come from (decision 0023). Sample data now;
//! a `ClientDataSource` on the `client` crate later, through the same trait.

use crate::model::shell::{
    Agent, AgentState, HostGroup, HostReach, ProjectGroup, SessionRow, ShellSnapshot,
    StatusBarModel, UsageModel, UsageStatus, WorkspaceRow,
};

/// Fills the shell's view models.
pub trait DataSource {
    /// Everything needed for the first paint.
    fn snapshot(&self) -> ShellSnapshot;
}

/// Fixed data that matches the shell artboards (`sh-empty-app`, `ws-session`).
#[derive(Clone, Copy, Debug, Default)]
pub struct SampleDataSource;

fn session(id: &str, title: &str, agent: Agent, state: AgentState) -> SessionRow {
    SessionRow {
        id: id.into(),
        title: title.into(),
        agent,
        state,
        place: None,
        own_worktree: false,
        can_bring_back: false,
        children: Vec::new(),
    }
}

impl DataSource for SampleDataSource {
    fn snapshot(&self) -> ShellSnapshot {
        let mut lead = session(
            "s1",
            "Implementation of master catalogs",
            Agent::Claude,
            AgentState::Running,
        );
        lead.children = vec![
            session("s1a", "Feed endpoints", Agent::Codex, AgentState::Waiting),
            session("s1b", "Catalog tests", Agent::GentleShell, AgentState::Done),
        ];
        ShellSnapshot {
            projects: vec![
                ProjectGroup {
                    name: "massorca-rust".into(),
                    hosts: vec![
                        HostGroup {
                            name: "This PC".into(),
                            reach: HostReach::Connected,
                            workspaces: vec![
                                WorkspaceRow {
                                    id: "w1".into(),
                                    name: "desktop-shell".into(),
                                    branch: Some("desktop/shell".into()),
                                    usage_slot: 1,
                                    sessions: vec![lead],
                                },
                                WorkspaceRow {
                                    id: "w2".into(),
                                    name: "core-store".into(),
                                    branch: Some("pr/store".into()),
                                    usage_slot: 2,
                                    sessions: vec![session(
                                        "s2",
                                        "SQLite migrations",
                                        Agent::Codex,
                                        AgentState::Idle,
                                    )],
                                },
                            ],
                        },
                        HostGroup {
                            name: "VPS".into(),
                            reach: HostReach::Unreachable { minutes: 5 },
                            workspaces: vec![WorkspaceRow {
                                id: "w3".into(),
                                name: "server-deploy".into(),
                                branch: Some("main".into()),
                                usage_slot: 3,
                                sessions: vec![session(
                                    "s3",
                                    "Headless server smoke test",
                                    Agent::Antigravity,
                                    AgentState::Unreachable,
                                )],
                            }],
                        },
                    ],
                },
                ProjectGroup {
                    name: "datalake".into(),
                    hosts: vec![HostGroup {
                        name: "This PC".into(),
                        reach: HostReach::Connected,
                        workspaces: vec![WorkspaceRow {
                            id: "w4".into(),
                            name: "gold-layer".into(),
                            branch: Some("feat/gold".into()),
                            usage_slot: 4,
                            sessions: vec![session(
                                "s4",
                                "Serve the gold tables",
                                Agent::Antigravity,
                                AgentState::Error,
                            )],
                        }],
                    }],
                },
            ],
            usage: vec![
                UsageModel {
                    agent: Agent::Claude,
                    status: UsageStatus::Percent(62),
                    slot: 1,
                    resets_in: "2 days".into(),
                },
                UsageModel {
                    agent: Agent::Codex,
                    status: UsageStatus::Percent(35),
                    slot: 2,
                    resets_in: "5 days".into(),
                },
                UsageModel {
                    agent: Agent::Antigravity,
                    status: UsageStatus::Percent(10),
                    slot: 3,
                    resets_in: "6 days".into(),
                },
            ],
            status: StatusBarModel {
                waiting: 1,
                running: 1,
                memory: Some("1.22 GB".into()),
                terminals: 2,
                ports: Some(0),
                hosts_total: 2,
                hosts_unreachable: 1,
            },
            session_title: Some("Implementation of master catalogs".into()),
            active_session: Some("s1".into()),
            prototypes: 3,
            service_status: None,
            service_notice: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_data_covers_every_agent_state_shown_in_the_sidebar() {
        let snap = SampleDataSource.snapshot();
        let mut states = Vec::new();
        fn walk(s: &SessionRow, out: &mut Vec<AgentState>) {
            out.push(s.state);
            s.children.iter().for_each(|c| walk(c, out));
        }
        for p in &snap.projects {
            for h in &p.hosts {
                for w in &h.workspaces {
                    w.sessions.iter().for_each(|s| walk(s, &mut states));
                }
            }
        }
        for s in [
            AgentState::Running,
            AgentState::Waiting,
            AgentState::Idle,
            AgentState::Error,
            AgentState::Unreachable,
            AgentState::Done,
        ] {
            assert!(states.contains(&s), "{s:?}");
        }
        assert_eq!(snap.usage.len(), 3, "three rings as in every artboard");
        let slots: Vec<u8> = snap
            .projects
            .iter()
            .flat_map(|p| &p.hosts)
            .flat_map(|h| &h.workspaces)
            .map(|w| w.usage_slot)
            .collect();
        assert!(
            slots.iter().all(|s| (1..=6).contains(s)),
            "usage colours only (P29)"
        );
    }
}
