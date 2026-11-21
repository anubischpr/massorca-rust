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
    /// Child sessions, shown indented under the parent (A83 to A85).
    pub children: Vec<SessionRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceRow {
    pub id: String,
    pub name: String,
    pub branch: Option<String>,
    /// Label colour, one of the six usage colours (1 to 6), never a status colour (P29).
    pub usage_slot: u8,
    pub sessions: Vec<SessionRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostGroup {
    pub name: String,
    pub reach: HostReach,
    pub workspaces: Vec<WorkspaceRow>,
}

/// A project is the top level of the Sidebar, with one host row per host (A10).
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectGroup {
    pub name: String,
    pub hosts: Vec<HostGroup>,
}

/// Usage of one provider, for the rail rings (D-SH-15).
#[derive(Clone, Debug, PartialEq)]
pub struct UsageModel {
    pub agent: Agent,
    /// The arc colour slot, 1 to 6, that the core assigns in order of first use.
    pub slot: u8,
    /// 0 to 100; `None` while unknown.
    pub percent: Option<u8>,
    /// Time until the window resets, such as "2 days".
    pub resets_in: String,
}

/// The Status bar items (D-SH-05).
#[derive(Clone, Debug, PartialEq)]
pub struct StatusBarModel {
    pub waiting: u32,
    pub running: u32,
    pub memory: String,
    pub terminals: u32,
    pub ports: u32,
    pub hosts_total: u32,
    pub hosts_unreachable: u32,
}

impl StatusBarModel {
    pub fn agents_text(&self) -> String {
        match (self.waiting, self.running) {
            (0, 0) => "No agents running".into(),
            (0, r) => format!("{r} running"),
            (w, _) => format!("{w} waiting for you"),
        }
    }

    pub fn terminals_text(&self) -> String {
        plural(self.terminals, "terminal", "No terminals")
    }

    pub fn ports_text(&self) -> String {
        plural(self.ports, "port", "No ports")
    }

    /// The agents item's tooltip, with the running count spelled out (D-SH-05).
    pub fn agents_tooltip(&self) -> String {
        match (self.waiting, self.running) {
            (0, 0) => "Agents: none running".into(),
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
        .flat_map(|host| &host.workspaces)
        .flat_map(|workspace| &workspace.sessions)
        .find_map(|session| find_in(session, id))
}

fn find_in<'a>(session: &'a SessionRow, id: &str) -> Option<&'a SessionRow> {
    if session.id == id {
        return Some(session);
    }
    session.children.iter().find_map(|child| find_in(child, id))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> StatusBarModel {
        StatusBarModel {
            waiting: 1,
            running: 2,
            memory: "1.22 GB".into(),
            terminals: 2,
            ports: 0,
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
    fn status_bar_copy_matches_the_shell_screen() {
        let mut s = status();
        assert_eq!(s.agents_text(), "1 waiting for you");
        assert_eq!(s.terminals_text(), "2 terminals");
        assert_eq!(s.ports_text(), "No ports");
        assert_eq!(s.hosts_text(), "2 hosts");
        s.hosts_unreachable = 1;
        assert_eq!(s.hosts_text(), "1 of 2 hosts unreachable");
        s.waiting = 0;
        assert_eq!(s.agents_text(), "2 running");
    }
}
