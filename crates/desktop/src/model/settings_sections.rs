//! The Settings navigation and its rows, as data (D-ST-01 to D-ST-12, section 6): the twelve
//! sections in nav order, the rows each one lists and what backs them, and search over them.
//! Free of GPUI, so the rules test without a window.

/// The service setting behind the workspace folder row.
pub const WORKSPACE_FOLDER_KEY: &str = "workspaces.default_root";
/// Where conversations without a project are kept (A95). The service has no setting for it yet.
pub const CONVERSATIONS_FOLDER: &str = "~/MassOrca/Conversations";
/// The control of a row that has no backing yet.
pub const NOT_AVAILABLE: &str = "Not available yet";

/// The twelve sections of the Settings navigation, in the order of the spec (section 6.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SettingsSection {
    #[default]
    General,
    Agents,
    Appearance,
    Input,
    Terminal,
    Sources,
    Hosts,
    Notifications,
    Extensions,
    Vault,
    Privacy,
    Routing,
}

impl SettingsSection {
    pub const ALL: [SettingsSection; 12] = [
        Self::General,
        Self::Agents,
        Self::Appearance,
        Self::Input,
        Self::Terminal,
        Self::Sources,
        Self::Hosts,
        Self::Notifications,
        Self::Extensions,
        Self::Vault,
        Self::Privacy,
        Self::Routing,
    ];

    /// The item's text, as the navigation shows it.
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General and account",
            Self::Agents => "Agents and accounts",
            Self::Appearance => "Appearance and layout",
            Self::Input => "Input and shortcuts",
            Self::Terminal => "Terminal and browser",
            Self::Sources => "Sources and integrations",
            Self::Hosts => "Hosts and phones",
            Self::Notifications => "Notifications and voice",
            Self::Extensions => "Extensions",
            Self::Vault => "Secret vault",
            Self::Privacy => "Privacy and advanced",
            Self::Routing => "Routing decisions",
        }
    }

    /// Extensions ends with an arrow: it opens Marketplace > Installed (A68), not a page here.
    pub fn opens_elsewhere(self) -> bool {
        self == Self::Extensions
    }
}

/// What a planned control does once it has a backing. It is drawn disabled until then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Planned {
    Switch,
    /// A text value, shown as it is.
    Text(&'static str),
    /// A short list of options; the value shown is the first one.
    Choice(&'static [&'static str]),
    Action(&'static str),
    Link,
    ReadOnly(&'static str),
}

/// What a row is: backed by the service or the bridge, or planned for a later layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowBody {
    /// `workspaces.default_root`, read and written through the service.
    WorkspaceFolder,
    /// The folder for conversations without a project. No service setting yet.
    ConversationsFolder,
    /// This PC and the state of its service.
    ThisPc,
    /// A link to the setup checklist.
    SetupChecklist,
    Planned(Planned),
}

/// One row of a section: its name, what it does, and what it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowSpec {
    pub label: &'static str,
    pub helper: &'static str,
    pub body: RowBody,
}

const GENERAL: &[RowSpec] = &[
    RowSpec {
        label: "Conversations folder",
        helper: "Where conversations without a project are saved.",
        body: RowBody::ConversationsFolder,
    },
    RowSpec {
        label: "Workspace folder",
        helper: "Folder for new workspaces when a project does not set its own root.",
        body: RowBody::WorkspaceFolder,
    },
    RowSpec {
        label: "MassOrca account",
        helper: "Sign in to use your account on this PC.",
        body: RowBody::Planned(Planned::Action("Sign in")),
    },
    RowSpec {
        label: "Updates",
        helper: "Release channel for MassOrca.",
        body: RowBody::Planned(Planned::Choice(&["Stable", "Preview"])),
    },
    RowSpec {
        label: "Setup checklist",
        helper: "Finish the steps that set this PC up.",
        body: RowBody::SetupChecklist,
    },
    RowSpec {
        label: "Support",
        helper: "Send feedback about MassOrca.",
        body: RowBody::Planned(Planned::Action("Send feedback")),
    },
];

const AGENTS: &[RowSpec] = &[
    RowSpec {
        label: "Default agent",
        helper: "The agent that starts a new session.",
        body: RowBody::Planned(Planned::Choice(&["Claude", "Codex", "OpenCode"])),
    },
    RowSpec {
        label: "Agent accounts and usage",
        helper: "Sign-in state and remaining usage for each agent.",
        body: RowBody::Planned(Planned::Link),
    },
    RowSpec {
        label: "Delegation depth limit",
        helper: "How many levels of child sessions an agent may start, from 1 to 5.",
        body: RowBody::Planned(Planned::Text("3")),
    },
    RowSpec {
        label: "Keep the computer awake",
        helper: "Stops the PC from sleeping while an agent runs.",
        body: RowBody::Planned(Planned::Switch),
    },
];

const APPEARANCE: &[RowSpec] = &[
    RowSpec {
        label: "Theme",
        helper: "Dark, light, or the same as the system.",
        body: RowBody::Planned(Planned::Choice(&["System", "Dark", "Light"])),
    },
    RowSpec {
        label: "Accent colour",
        helper: "Lavender is the default accent.",
        body: RowBody::Planned(Planned::Action("Choose")),
    },
    RowSpec {
        label: "Density",
        helper: "How much space rows take.",
        body: RowBody::Planned(Planned::Choice(&["Compact", "Default", "Comfortable"])),
    },
    RowSpec {
        label: "Text size",
        helper: "Scales the interface text from 90% to 130%.",
        body: RowBody::Planned(Planned::Text("100%")),
    },
    RowSpec {
        label: "Session summary",
        helper: "Shows only the cards that have information.",
        body: RowBody::Planned(Planned::Switch),
    },
];

const INPUT: &[RowSpec] = &[
    RowSpec {
        label: "Tab width",
        helper: "Spaces inserted for a tab in the code editor.",
        body: RowBody::Planned(Planned::Text("4")),
    },
    RowSpec {
        label: "Keyboard shortcuts",
        helper: "View and change the shortcut for each action.",
        body: RowBody::Planned(Planned::Action("Edit shortcuts")),
    },
];

const TERMINAL: &[RowSpec] = &[
    RowSpec {
        label: "Terminal font",
        helper: "Font used in terminal panes.",
        body: RowBody::Planned(Planned::Text("Cascadia Mono")),
    },
    RowSpec {
        label: "Scrollback lines",
        helper: "How many lines a terminal keeps.",
        body: RowBody::Planned(Planned::Text("10000")),
    },
    RowSpec {
        label: "Shell",
        helper: "The shell a new terminal starts.",
        body: RowBody::Planned(Planned::Text("PowerShell")),
    },
    RowSpec {
        label: "Browser home page",
        helper: "The page a new browser tab opens.",
        body: RowBody::Planned(Planned::Text("about:blank")),
    },
];

const SOURCES: &[RowSpec] = &[
    RowSpec {
        label: "Git providers",
        helper: "Connecting needs your grant on the provider's own page.",
        body: RowBody::Planned(Planned::Action("Connect")),
    },
    RowSpec {
        label: "Linear",
        helper: "Lets an agent read and update Linear issues.",
        body: RowBody::Planned(Planned::Action("Connect")),
    },
    RowSpec {
        label: "Fetch on focus",
        helper: "Fetches the remote when a workspace gains focus.",
        body: RowBody::Planned(Planned::Switch),
    },
];

const HOSTS: &[RowSpec] = &[
    RowSpec {
        label: "This PC",
        helper: "The MassOrca service on this computer.",
        body: RowBody::ThisPc,
    },
    RowSpec {
        label: "Add host",
        helper: "Remote hosts over SSH come later.",
        body: RowBody::Planned(Planned::Action("Add host")),
    },
    RowSpec {
        label: "Paired phones",
        helper: "Phone pairing over the local network comes later.",
        body: RowBody::Planned(Planned::Link),
    },
];

const NOTIFICATIONS: &[RowSpec] = &[
    RowSpec {
        label: "Agent finished",
        helper: "Tells you when an agent ends a run you started.",
        body: RowBody::Planned(Planned::Switch),
    },
    RowSpec {
        label: "Waiting for you",
        helper: "Tells you when an agent needs an answer.",
        body: RowBody::Planned(Planned::Switch),
    },
    RowSpec {
        label: "Dictation",
        helper: "Turns speech into text in the composer.",
        body: RowBody::Planned(Planned::Switch),
    },
];

const EXTENSIONS: &[RowSpec] = &[RowSpec {
    label: "Extensions",
    helper: "Managed in Marketplace, under Installed.",
    body: RowBody::Planned(Planned::Link),
}];

const VAULT: &[RowSpec] = &[RowSpec {
    label: "Secrets",
    helper: "Values never go into a chat, a prompt or a log.",
    body: RowBody::Planned(Planned::Action("Add secret")),
}];

const PRIVACY: &[RowSpec] = &[
    RowSpec {
        label: "Crash reports",
        helper: "Off unless you turn it on. A preview comes before anything is sent.",
        body: RowBody::Planned(Planned::Switch),
    },
    RowSpec {
        label: "Diagnostic export",
        helper: "Saves logs to a file you choose.",
        body: RowBody::Planned(Planned::Action("Export")),
    },
    RowSpec {
        label: "Network proxy",
        helper: "Address of a proxy server, if this network needs one.",
        body: RowBody::Planned(Planned::Text("None")),
    },
];

const ROUTING: &[RowSpec] = &[RowSpec {
    label: "Routing decisions",
    helper: "Why each Bot Teams request went to the bot it did.",
    body: RowBody::Planned(Planned::ReadOnly("None yet")),
}];

/// The rows a section lists, in order.
pub fn rows(section: SettingsSection) -> &'static [RowSpec] {
    match section {
        SettingsSection::General => GENERAL,
        SettingsSection::Agents => AGENTS,
        SettingsSection::Appearance => APPEARANCE,
        SettingsSection::Input => INPUT,
        SettingsSection::Terminal => TERMINAL,
        SettingsSection::Sources => SOURCES,
        SettingsSection::Hosts => HOSTS,
        SettingsSection::Notifications => NOTIFICATIONS,
        SettingsSection::Extensions => EXTENSIONS,
        SettingsSection::Vault => VAULT,
        SettingsSection::Privacy => PRIVACY,
        SettingsSection::Routing => ROUTING,
    }
}

/// One row that matches a search: its section and its position in that section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub section: SettingsSection,
    pub index: usize,
}

impl Hit {
    pub fn row(self) -> &'static RowSpec {
        &rows(self.section)[self.index]
    }
}

/// Every row whose name, helper or section name contains the query, ignoring case, in nav order.
/// An empty query matches nothing: the page then shows the selected section.
pub fn search(query: &str) -> Vec<Hit> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.as_str();
    SettingsSection::ALL
        .into_iter()
        .flat_map(|section| {
            rows(section)
                .iter()
                .enumerate()
                .filter(move |(_, row)| {
                    [row.label, row.helper, section.label()]
                        .iter()
                        .any(|text| text.to_lowercase().contains(needle))
                })
                .map(move |(index, _)| Hit { section, index })
        })
        .collect()
}

/// The count under the search field: "3 matches in this section" when every hit is in the
/// selected section, otherwise "4 matches in 3 sections".
pub fn hit_line(hits: &[Hit], selected: SettingsSection) -> String {
    let mut sections: Vec<SettingsSection> = hits.iter().map(|hit| hit.section).collect();
    sections.sort_by_key(|section| *section as usize);
    sections.dedup();
    let count = hits.len();
    let noun = if count == 1 { "match" } else { "matches" };
    if sections.iter().all(|section| *section == selected) {
        format!("{count} {noun} in this section")
    } else {
        let sections_count = sections.len();
        let section_noun = if sections_count == 1 {
            "section"
        } else {
            "sections"
        };
        format!("{count} {noun} in {sections_count} {section_noun}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_twelve_sections_keep_the_spec_order_and_names() {
        assert_eq!(SettingsSection::ALL.len(), 12);
        assert_eq!(SettingsSection::ALL[0].label(), "General and account");
        assert_eq!(SettingsSection::ALL[8].label(), "Extensions");
        assert_eq!(SettingsSection::ALL[11].label(), "Routing decisions");
        assert!(SettingsSection::Extensions.opens_elsewhere());
        assert!(!SettingsSection::General.opens_elsewhere());
    }

    #[test]
    fn every_section_lists_at_least_one_row_and_only_general_holds_the_folder_setting() {
        for section in SettingsSection::ALL {
            assert!(!rows(section).is_empty(), "{section:?}");
        }
        let folders: Vec<_> = SettingsSection::ALL
            .into_iter()
            .flat_map(|section| rows(section).iter().map(move |row| (section, row.body)))
            .filter(|(_, body)| *body == RowBody::WorkspaceFolder)
            .collect();
        assert_eq!(
            folders,
            vec![(SettingsSection::General, RowBody::WorkspaceFolder)]
        );
    }

    #[test]
    fn search_ignores_case_and_looks_at_names_helpers_and_section_names() {
        let theme: Vec<&str> = search("THEME").iter().map(|hit| hit.row().label).collect();
        assert_eq!(theme, vec!["Theme"]);
        let shortcut: Vec<&str> = search("shortcut")
            .iter()
            .map(|hit| hit.row().label)
            .collect();
        // The section name "Input and shortcuts" matches too, so its other rows come along.
        assert!(shortcut.contains(&"Keyboard shortcuts"));
        let by_section = search("appearance");
        assert_eq!(by_section.len(), rows(SettingsSection::Appearance).len());
    }

    #[test]
    fn an_empty_or_unknown_query_has_no_hits() {
        assert!(search("   ").is_empty());
        assert!(search("xyz").is_empty());
    }

    #[test]
    fn the_hit_line_counts_in_this_section_or_in_sections() {
        let theme = search("theme");
        assert_eq!(
            hit_line(&theme, SettingsSection::Appearance),
            "1 match in this section"
        );
        assert_eq!(
            hit_line(&theme, SettingsSection::General),
            "1 match in 1 section"
        );
        let spread = search("on");
        assert!(hit_line(&spread, SettingsSection::General).ends_with("sections"));
    }
}
