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

/// Everything the shell shows at one moment.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellSnapshot {
    pub projects: Vec<ProjectGroup>,
    pub usage: Vec<UsageModel>,
    pub status: StatusBarModel,
    /// Title of the open session, if any.
    pub session_title: Option<String>,
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
