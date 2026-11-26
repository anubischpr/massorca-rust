//! Where the shell's view models come from (decision 0023). Sample data now;
//! a `ClientDataSource` on the `client` crate later, through the same trait.

use crate::model::project_icon::ProjectGlyph;
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

/// A sample workspace: its id, its branch (None for a folder) and its path.
#[derive(Clone, Copy)]
struct Ws(&'static str, Option<&'static str>, &'static str);

fn session(ws: Ws, id: &str, title: &str, agent: Agent, state: AgentState) -> SessionRow {
    SessionRow {
        id: id.into(),
        title: title.into(),
        agent,
        state,
        workspace_id: ws.0.into(),
        terminal_id: None,
        branch: ws.1.map(Into::into),
        path: Some(ws.2.into()),
        place: None,
        own_worktree: false,
        can_bring_back: false,
        unread: false,
        children: Vec::new(),
    }
}

fn workspace(ws: Ws, name: &str, usage_slot: u8) -> WorkspaceRow {
    WorkspaceRow {
        id: ws.0.into(),
        name: name.into(),
        branch: ws.1.map(Into::into),
        usage_slot,
    }
}

impl DataSource for SampleDataSource {
    fn snapshot(&self) -> ShellSnapshot {
        let shell = Ws("w1", Some("desktop/shell"), "D:/work/desktop-shell");
        let store = Ws("w2", Some("pr/store"), "D:/work/core-store");
        let deploy = Ws("w3", Some("main"), "/srv/server-deploy");
        let gold = Ws("w4", Some("feat/gold"), "D:/work/gold-layer");
        let mut lead = session(
            shell,
            "s1",
            "Implementation of master catalogs",
            Agent::Claude,
            AgentState::Running,
        );
        lead.children = vec![
            session(
                shell,
                "s1a",
                "Feed endpoints",
                Agent::Codex,
                AgentState::Waiting,
            ),
            session(
                shell,
                "s1b",
                "Catalog tests",
                Agent::GentleShell,
                AgentState::Done,
            ),
        ];
        ShellSnapshot {
            projects: vec![
                ProjectGroup {
                    icon: ProjectGlyph::Folder,
                    owner: None,
                    plain_folder: false,
                    id: "massorca-rust".into(),
                    name: "massorca-rust".into(),
                    hosts: vec![
                        HostGroup {
                            name: "This PC".into(),
                            reach: HostReach::Connected,
                            workspaces: vec![
                                workspace(shell, "desktop-shell", 1),
                                workspace(store, "core-store", 2),
                            ],
                            sessions: vec![
                                lead,
                                session(
                                    store,
                                    "s2",
                                    "SQLite migrations",
                                    Agent::Codex,
                                    AgentState::Idle,
                                ),
                            ],
                            placeholders: Vec::new(),
                        },
                        HostGroup {
                            name: "VPS".into(),
                            reach: HostReach::Unreachable { minutes: 5 },
                            workspaces: vec![workspace(deploy, "server-deploy", 3)],
                            sessions: vec![session(
                                deploy,
                                "s3",
                                "Headless server smoke test",
                                Agent::Antigravity,
                                AgentState::Unreachable,
                            )],
                            placeholders: Vec::new(),
                        },
                    ],
                },
                ProjectGroup {
                    icon: ProjectGlyph::Folder,
                    owner: None,
                    plain_folder: false,
                    id: "datalake".into(),
                    name: "datalake".into(),
                    hosts: vec![HostGroup {
                        name: "This PC".into(),
                        reach: HostReach::Connected,
                        workspaces: vec![workspace(gold, "gold-layer", 4)],
                        sessions: vec![session(
                            gold,
                            "s4",
                            "Serve the gold tables",
                            Agent::Antigravity,
                            AgentState::Error,
                        )],
                        placeholders: Vec::new(),
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
            preflight: None,
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
                h.sessions.iter().for_each(|s| walk(s, &mut states));
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
