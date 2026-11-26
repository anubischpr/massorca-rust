//! The view models the shell renders from (decision 0023). A `DataSource`
//! fills them; views never read anything else.

/// An agent the app can run. Mapped to `massorca_ui` icons by the views.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Agent {
    Claude,
    Codex,
    Antigravity,
    GentleShell,
    Gemini,
    OpenCode,
    Cursor,
    /// A kind the desktop has no mark for, such as one the profile declares.
    Unknown,
}

/// The six agent states (P30).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentState {
    Running,
    Waiting,
    Idle,
    Error,
    Unreachable,
    Done,
}

impl AgentState {}

/// Whether the app can reach a host right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostReach {
    Connected,
    /// Unreachable since this many minutes (honest state: show the age).
    Unreachable {
        minutes: u32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionRow {
    pub id: String,
    pub title: String,
    pub agent: Agent,
    pub state: AgentState,
    /// The workspace the session runs in. It shows in the row's tooltip, never as a row (A97).
    pub workspace_id: String,
    /// The branch of the session's worktree; None for a plain folder.
    pub branch: Option<String>,
    /// The worktree's path, or the folder's path.
    pub path: Option<String>,
    /// Where a child session runs: "Current worktree", or the branch of its own worktree. None for a top session.
    pub place: Option<String>,
    /// A child that runs in another worktree than its parent's; its row shows a branch icon (WS-35).
    pub own_worktree: bool,
    /// Whether the row offers to bring this child back: it has finished and has not been brought back yet.
    pub can_bring_back: bool,
    /// The agent's latest `waiting` or `done` has not been marked seen (WS-33); never folded.
    pub unread: bool,
    /// Child sessions, shown indented under the parent (A83 to A85).
    pub children: Vec<SessionRow>,
}

/// A workspace with no session yet, drawn as one session row with the idle glyph until its first session replaces
/// it (A97).
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceholderRow {
    pub workspace_id: String,
    /// The branch of the workspace's worktree, or the folder's name for a plain folder.
    pub title: String,
    /// The chosen agent's mark, else the configured default's, else `Agent::Unknown` (a neutral glyph).
    pub agent: Agent,
    pub branch: Option<String>,
    /// The worktree's path, or the folder's path.
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceRow {
    pub id: String,
    pub name: String,
    pub branch: Option<String>,
    /// Label colour, one of the six usage colours (1 to 6), never a status colour (P29).
    pub usage_slot: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostGroup {
    pub name: String,
    pub reach: HostReach,
    /// The workspaces the host has, for the palette. The Sidebar lists none of them (A97).
    pub workspaces: Vec<WorkspaceRow>,
    /// The top sessions the host runs, oldest first, each with its children below it (A97).
    pub sessions: Vec<SessionRow>,
    /// The workspaces with no session yet, listed after the sessions where the first one will appear (A97).
    pub placeholders: Vec<PlaceholderRow>,
}

/// A project is the top level of the Sidebar, with one host row per host (A10).
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectGroup {
    pub id: String,
    pub name: String,
    /// What the project's row and pickers draw (decision 0091).
    pub icon: crate::model::project_icon::ProjectGlyph,
    /// The owner or org of the project's remote, drawn after its name (A10); None until the protocol carries one.
    pub owner: Option<String>,
    /// A folder that is not a Git repository: its row shows the No Git marker (WS-02).
    pub plain_folder: bool,
    pub hosts: Vec<HostGroup>,
}

/// What the shell knows about one provider's usage (D-SH-15, UsageRing.md states).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageStatus {
    /// A reported percentage, 0 to 100.
    Percent(u8),
    /// Asked for and not back yet.
    Loading,
    /// The provider reports no usage.
    NotReported,
    /// The usage cannot be had.
    Unavailable,
    /// The reading failed; the last value stays.
    Failed { last: Option<u8> },
}

impl UsageStatus {
    /// The line the usage popover leads with. `None` while loading, when a skeleton shows.
    pub fn headline(self) -> Option<String> {
        match self {
            Self::Percent(percent) => Some(format!("{percent} percent")),
            Self::Loading => None,
            Self::NotReported => Some("Usage not reported".into()),
            Self::Unavailable => Some("Usage unavailable".into()),
            Self::Failed {
                last: Some(percent),
            } => Some(format!(
                "Usage could not be read. Last value {percent} percent"
            )),
            Self::Failed { last: None } => Some("Usage could not be read".into()),
        }
    }
}

/// Usage of one provider, for the rail rings (D-SH-15).
#[derive(Clone, Debug, PartialEq)]
pub struct UsageModel {
    pub agent: Agent,
    /// The arc colour slot, 1 to 6, that the core assigns in order of first use.
    pub slot: u8,
    pub status: UsageStatus,
    /// Time until the window resets, such as "2 days". Empty when the provider does not say.
    pub resets_in: String,
}

/// The Status bar items (D-SH-05). Memory and ports are `None` when no source reports them: their
/// items are then hidden rather than shown with a placeholder.
#[derive(Clone, Debug, PartialEq)]
pub struct StatusBarModel {
    pub waiting: u32,
    pub running: u32,
    pub memory: Option<String>,
    pub terminals: u32,
    pub ports: Option<u32>,
    pub hosts_total: u32,
    pub hosts_unreachable: u32,
}

impl StatusBarModel {
    pub fn agents_text(&self) -> String {
        match (self.waiting, self.running) {
            (0, 0) => "Agents: idle".into(),
            (0, r) => format!("{r} running"),
            (w, _) => format!("{w} waiting for you"),
        }
    }

    /// The Memory item carries its label, as sh-empty-app draws it: "Memory 1.22 GB".
    pub fn memory_text(&self) -> Option<String> {
        self.memory
            .as_ref()
            .map(|memory| format!("Memory {memory}"))
    }

    /// The Bot Teams copy of the terminals item: a count, as bt-board draws it.
    pub fn bot_teams_terminals_text(&self) -> String {
        format!("Terminals {}", self.terminals)
    }

    /// The Bot Teams copy of the ports item: a count, as bt-board draws it.
    pub fn bot_teams_ports_text(&self) -> Option<String> {
        self.ports.map(|ports| format!("Ports {ports}"))
    }

    /// The Bot Teams copy of the hosts item: only the unreachable hosts are named, as bt-board draws it.
    pub fn bot_teams_hosts_text(&self) -> String {
        if self.hosts_unreachable == 0 {
            return self.hosts_text();
        }
        let noun = if self.hosts_unreachable == 1 {
            "host"
        } else {
            "hosts"
        };
        format!("{} {noun} unreachable", self.hosts_unreachable)
    }

    pub fn terminals_text(&self) -> String {
        plural(self.terminals, "terminal", "No terminals")
    }

    pub fn ports_text(&self) -> Option<String> {
        self.ports.map(|ports| plural(ports, "port", "No ports"))
    }

    /// The agents item's tooltip, with the running count spelled out (D-SH-05).
    pub fn agents_tooltip(&self) -> String {
        match (self.waiting, self.running) {
            (0, 0) => "Agents: idle".into(),
            (0, r) => format!("Agents: {r} running"),
            (w, 0) => format!("Agents: {w} waiting for you"),
            (w, r) => format!("Agents: {w} waiting for you, {r} running"),
        }
    }

    /// The hosts item's tooltip, which names the disconnected count.
    pub fn hosts_tooltip(&self) -> String {
        if self.hosts_unreachable > 0 {
            format!(
                "Hosts: {} connected, {} disconnected",
                self.hosts_total.saturating_sub(self.hosts_unreachable),
                self.hosts_unreachable
            )
        } else {
            format!("Hosts: {} connected", self.hosts_total)
        }
    }

    pub fn hosts_text(&self) -> String {
        if self.hosts_unreachable > 0 {
            format!(
                "{} of {} hosts unreachable",
                self.hosts_unreachable, self.hosts_total
            )
        } else {
            plural(self.hosts_total, "host", "No hosts")
        }
    }
}

fn plural(n: u32, word: &str, none: &str) -> String {
    match n {
        0 => none.into(),
        1 => format!("1 {word}"),
        n => format!("{n} {word}s"),
    }
}

/// The session with this id, searched through every project, host, workspace and child.
pub fn find_session<'a>(projects: &'a [ProjectGroup], id: &str) -> Option<&'a SessionRow> {
    projects
        .iter()
        .flat_map(|project| &project.hosts)
        .flat_map(|host| &host.sessions)
        .find_map(|session| find_in(session, id))
}

fn find_in<'a>(session: &'a SessionRow, id: &str) -> Option<&'a SessionRow> {
    if session.id == id {
        return Some(session);
    }
    session.children.iter().find_map(|child| find_in(child, id))
}

/// How the link to the service looks, which sets the colour of its status item and notice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceTone {
    Ok,
    Busy,
    Warning,
    Unreachable,
}

/// The Status bar's service item (decision 0028).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceStatus {
    pub tone: ServiceTone,
    pub text: String,
    pub tooltip: String,
}

/// The banner above the Canvas while the service is not connected (X-CN-01).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceNotice {
    pub tone: ServiceTone,
    pub message: String,
    /// The reason or the retry timer, behind the banner's Details.
    pub detail: Option<String>,
    /// Whether the banner offers "Retry now".
    pub retry: bool,
}

/// Everything the shell shows at one moment.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellSnapshot {
    pub projects: Vec<ProjectGroup>,
    pub usage: Vec<UsageModel>,
    pub status: StatusBarModel,
    /// Title of the open session, if any.
    pub session_title: Option<String>,
    /// Id of the open session, which the Sidebar marks as selected.
    pub active_session: Option<String>,
    /// How many prototypes the Workspaces navigation counts (D-SH-01).
    pub prototypes: u32,
    /// The service link, when the data comes from a service. Sample data has none.
    pub service_status: Option<ServiceStatus>,
    pub service_notice: Option<ServiceNotice>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> StatusBarModel {
        StatusBarModel {
            waiting: 1,
            running: 2,
            memory: Some("1.22 GB".into()),
            terminals: 2,
            ports: Some(0),
            hosts_total: 2,
            hosts_unreachable: 0,
        }
    }

    #[test]
    fn find_session_reaches_child_sessions() {
        use crate::data::{DataSource, SampleDataSource};
        let snap = SampleDataSource.snapshot();
        assert_eq!(
            find_session(&snap.projects, "s1b").map(|s| s.title.as_str()),
            Some("Catalog tests")
        );
        assert!(find_session(&snap.projects, "missing").is_none());
    }

    #[test]
    fn status_tooltips_spell_out_the_counts() {
        let mut s = status();
        assert_eq!(s.agents_tooltip(), "Agents: 1 waiting for you, 2 running");
        assert_eq!(s.hosts_tooltip(), "Hosts: 2 connected");
        s.hosts_unreachable = 1;
        assert_eq!(s.hosts_tooltip(), "Hosts: 1 connected, 1 disconnected");
    }

    #[test]
    fn an_idle_agents_item_reads_agents_idle_as_the_shell_screen_does() {
        let mut s = status();
        s.waiting = 0;
        s.running = 0;
        assert_eq!(s.agents_text(), "Agents: idle");
        assert_eq!(s.agents_tooltip(), "Agents: idle");
    }

    #[test]
    fn the_memory_item_names_its_value() {
        assert_eq!(status().memory_text(), Some("Memory 1.22 GB".into()));
    }

    #[test]
    fn memory_and_ports_have_no_copy_without_a_source() {
        let mut s = status();
        s.memory = None;
        s.ports = None;
        assert_eq!(s.memory_text(), None);
        assert_eq!(s.ports_text(), None);
        assert_eq!(s.bot_teams_ports_text(), None);
    }

    #[test]
    fn bot_teams_copy_counts_terminals_ports_and_unreachable_hosts() {
        let mut s = status();
        s.terminals = 0;
        assert_eq!(s.bot_teams_terminals_text(), "Terminals 0");
        assert_eq!(s.bot_teams_ports_text(), Some("Ports 0".into()));
        s.terminals = 2;
        assert_eq!(s.bot_teams_terminals_text(), "Terminals 2");
        assert_eq!(s.bot_teams_hosts_text(), "2 hosts");
        s.hosts_unreachable = 1;
        assert_eq!(s.bot_teams_hosts_text(), "1 host unreachable");
        s.hosts_unreachable = 2;
        assert_eq!(s.bot_teams_hosts_text(), "2 hosts unreachable");
    }

    #[test]
    fn status_bar_copy_matches_the_shell_screen() {
        let mut s = status();
        assert_eq!(s.agents_text(), "1 waiting for you");
        assert_eq!(s.terminals_text(), "2 terminals");
        assert_eq!(s.ports_text(), Some("No ports".into()));
        assert_eq!(s.hosts_text(), "2 hosts");
        s.hosts_unreachable = 1;
        assert_eq!(s.hosts_text(), "1 of 2 hosts unreachable");
        s.waiting = 0;
        assert_eq!(s.agents_text(), "2 running");
    }

    #[test]
    fn usage_copy_follows_the_ring_states() {
        assert_eq!(
            UsageStatus::Percent(62).headline().as_deref(),
            Some("62 percent")
        );
        assert_eq!(UsageStatus::Loading.headline(), None);
        assert_eq!(
            UsageStatus::NotReported.headline().as_deref(),
            Some("Usage not reported")
        );
        assert_eq!(
            UsageStatus::Unavailable.headline().as_deref(),
            Some("Usage unavailable")
        );
        assert_eq!(
            UsageStatus::Failed { last: Some(62) }.headline().as_deref(),
            Some("Usage could not be read. Last value 62 percent")
        );
        assert_eq!(
            UsageStatus::Failed { last: None }.headline().as_deref(),
            Some("Usage could not be read")
        );
    }
}
