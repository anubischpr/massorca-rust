//! The Settings navigation and its rows, as data (D-ST-01 to D-ST-12, section 6): the twelve
//! sections in nav order, the rows each one lists and what backs them, and search over them.
//! Free of GPUI, so the rules test without a window.

use massorca_ui::assets::{FONT_MONO, FONT_SANS};

/// The service setting behind the workspace folder row.
pub const WORKSPACE_FOLDER_KEY: &str = "workspaces.default_root";
/// Where conversations without a project are kept (A95). The service has no setting for it yet.
pub const CONVERSATIONS_FOLDER: &str = "~/MassOrca/Conversations";
/// The status of a row whose layer is not built yet.
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

    /// The line under the section's title, where the section's rows depend on a grant.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::Sources => {
                Some("Connecting a provider needs your grant on the provider's own page.")
            }
            Self::Vault => Some(
                "Values never go into a chat, a prompt or a log. Sharing creates a link, not a copy: rotate the owner's value once and every linked scope uses it.",
            ),
            _ => None,
        }
    }
}

/// What a planned control does once it has a backing. It is drawn disabled until then, and holds
/// or sends nothing. A value is shown only where the design documents it as the default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Planned {
    /// An on-off switch.
    Switch,
    /// Short segmented options; the first one is shown as the choice.
    Choice(&'static [&'static str]),
    /// A dropdown, with the default shown when there is one.
    Select(Option<&'static str>),
    /// A dropdown and a ghost action beside it.
    SelectAction(Option<&'static str>, &'static str),
    /// Segmented options and a button beside them.
    ChoiceAction(&'static [&'static str], &'static str),
    /// A text field, with the default shown when there is one.
    Text(Option<&'static str>),
    /// A number field with its stepper and unit, with the default shown when there is one.
    Number {
        value: Option<&'static str>,
        unit: &'static str,
    },
    /// A slider that shows `value`, with its knob at `percent` of the way along.
    Slider { value: &'static str, percent: u8 },
    /// The six accent presets and a custom colour.
    Swatches,
    /// One checkbox per option, each shown.
    Checkboxes(&'static [&'static str]),
    /// A switch and a sound dropdown, for one notification source.
    SwitchSelect,
    /// A table with these columns, and no rows yet.
    Table(&'static [&'static str]),
    /// A button.
    Action(&'static str),
    /// A link row that opens a sub-page.
    Link,
    /// A link row to a page that is not built yet; the text says what is missing.
    Elsewhere(&'static str),
    /// A row with no control; its status says what is missing.
    Status,
}

/// What a row is: backed by the service or the bridge, or planned for a later layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowBody {
    /// `workspaces.default_root`, read and written through the service.
    WorkspaceFolder,
    /// `appearance.theme`: the three choices, read and written through the service.
    AppTheme,
    /// The folder for conversations without a project. No service setting yet.
    ConversationsFolder,
    /// This PC and the state of its service.
    ThisPc,
    /// Each paired remote host, one row per host, from the store (D-HO-07). The spec only marks the place.
    PairedHosts,
    /// A link to the setup checklist.
    SetupChecklist,
    /// A link to the Pairing page: phones and other hosts, and the devices paired with this PC.
    PairingLink,
    /// Opens the Add host dialog, its SSH path (D-HO-02).
    AddHost,
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
        label: "Conversations without a project",
        helper: "Where conversations without a project are saved.",
        body: RowBody::ConversationsFolder,
    },
    RowSpec {
        label: "Workspace folder",
        helper: "Folder for new workspaces when a project does not set its own root.",
        body: RowBody::WorkspaceFolder,
    },
    RowSpec {
        label: "Editor defaults",
        helper: "The editor that opens a workspace, and its defaults.",
        body: RowBody::Planned(Planned::Action("Choose")),
    },
    RowSpec {
        label: "MassOrca account",
        helper: "Sign in to use your account on this PC.",
        body: RowBody::Planned(Planned::Action("Sign in")),
    },
    RowSpec {
        label: "Updates",
        helper: "Release channel for MassOrca.",
        body: RowBody::Planned(Planned::ChoiceAction(
            &["Stable", "Preview"],
            "Check for updates",
        )),
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
        body: RowBody::Planned(Planned::Select(None)),
    },
    RowSpec {
        label: "Computer use",
        helper: "Lets an agent use the screen, mouse and keyboard.",
        body: RowBody::Planned(Planned::Switch),
    },
    RowSpec {
        label: "Browser access",
        helper: "Lets an agent drive the browser.",
        body: RowBody::Planned(Planned::Switch),
    },
    RowSpec {
        label: "Keep the computer awake",
        helper: "Stops the PC from sleeping while an agent runs.",
        body: RowBody::Planned(Planned::Switch),
    },
    RowSpec {
        label: "Session search",
        helper: "Searches inside past agent sessions.",
        body: RowBody::Planned(Planned::Switch),
    },
    RowSpec {
        label: "Delegation depth limit",
        helper: "How many levels of child sessions an agent may start, from 1 to 5.",
        body: RowBody::Planned(Planned::Number {
            value: Some("3"),
            unit: "levels",
        }),
    },
];

const APPEARANCE: &[RowSpec] = &[
    RowSpec {
        label: "Theme",
        helper: "Dark, light, or the same as the system.",
        body: RowBody::AppTheme,
    },
    RowSpec {
        label: "Import theme",
        helper: "Warp and Ghostty themes. A theme below the contrast minimum is rejected.",
        body: RowBody::Planned(Planned::Action("Import")),
    },
    RowSpec {
        label: "Layout preset",
        helper: "Switch between saved layouts, or save this one.",
        body: RowBody::Planned(Planned::SelectAction(None, "Save as preset")),
    },
    RowSpec {
        label: "Accent colour",
        helper: "Lavender is the default accent.",
        body: RowBody::Planned(Planned::Swatches),
    },
    RowSpec {
        label: "Density",
        helper: "How much space rows take.",
        body: RowBody::Planned(Planned::Choice(&["Compact", "Default", "Comfortable"])),
    },
    RowSpec {
        label: "Text size",
        helper: "Scales the interface text from 90% to 130%.",
        body: RowBody::Planned(Planned::Slider {
            value: "100%",
            percent: 25,
        }),
    },
    RowSpec {
        label: "Interface font",
        helper: "Font for the interface.",
        body: RowBody::Planned(Planned::Select(Some(FONT_SANS))),
    },
    RowSpec {
        label: "Monospace font",
        helper: "Font for code, terminals and paths.",
        body: RowBody::Planned(Planned::Select(Some(FONT_MONO))),
    },
    RowSpec {
        label: "Session summary",
        helper: "Shows only the cards that have information.",
        body: RowBody::Planned(Planned::Switch),
    },
    RowSpec {
        label: "Status bar items",
        helper: "What the status bar shows.",
        body: RowBody::Planned(Planned::Checkboxes(&[
            "Agent status",
            "Memory",
            "Terminals",
            "Ports",
            "Hosts",
        ])),
    },
];

const INPUT: &[RowSpec] = &[
    RowSpec {
        label: "Tab width",
        helper: "Spaces inserted for a tab in the code editor.",
        body: RowBody::Planned(Planned::Number {
            value: Some("4"),
            unit: "spaces",
        }),
    },
    RowSpec {
        label: "Shortcut filter",
        helper: "Finds an action by its name or its shortcut.",
        body: RowBody::Planned(Planned::Text(None)),
    },
    RowSpec {
        label: "Shortcuts to show",
        helper: "Every shortcut, only the conflicts, or the ones extensions add.",
        body: RowBody::Planned(Planned::Choice(&["All", "Conflicts", "Extensions"])),
    },
    RowSpec {
        label: "Reset all",
        helper: "Puts every shortcut back to its default.",
        body: RowBody::Planned(Planned::Action("Reset all")),
    },
    RowSpec {
        label: "Keyboard shortcuts",
        helper: "View and change the shortcut for each action.",
        body: RowBody::Planned(Planned::Table(&["Action", "From", "Shortcut"])),
    },
    RowSpec {
        label: "Quick commands",
        helper: "Saved commands you can run from the palette.",
        body: RowBody::Planned(Planned::Action("Manage")),
    },
];

const TERMINAL: &[RowSpec] = &[
    RowSpec {
        label: "Terminal font",
        helper: "Font used in terminal panes.",
        body: RowBody::Planned(Planned::Select(Some(FONT_MONO))),
    },
    RowSpec {
        label: "Terminal palette",
        helper: "Colours for terminal panes. Warp and Ghostty themes can be imported.",
        body: RowBody::Planned(Planned::Select(None)),
    },
    RowSpec {
        label: "Scrollback lines",
        helper: "How many lines a terminal keeps.",
        // The terminal keeps SCROLLBACK_ROWS rows by default (crates/core-terminal).
        body: RowBody::Planned(Planned::Number {
            value: Some("1000"),
            unit: "lines",
        }),
    },
    RowSpec {
        label: "Shell",
        helper: "The shell a new terminal starts.",
        body: RowBody::Planned(Planned::Select(None)),
    },
    RowSpec {
        label: "Rendering",
        helper: "How terminal text is drawn.",
        body: RowBody::Planned(Planned::Select(None)),
    },
    RowSpec {
        label: "Browser home page",
        helper: "The page a new browser tab opens.",
        body: RowBody::Planned(Planned::Text(None)),
    },
    RowSpec {
        label: "Search engine",
        helper: "Used when you search from the browser's address bar.",
        body: RowBody::Planned(Planned::Select(None)),
    },
    RowSpec {
        label: "Default zoom",
        helper: "Zoom for a new browser page.",
        body: RowBody::Planned(Planned::Number {
            value: Some("100"),
            unit: "%",
        }),
    },
    RowSpec {
        label: "Browser profiles",
        helper: "Separate profiles, each with its own sign-ins.",
        body: RowBody::Planned(Planned::Link),
    },
    RowSpec {
        label: "Link routing",
        helper: "Chooses where links open: the browser, the editor or another app.",
        body: RowBody::Planned(Planned::Select(None)),
    },
];

const SOURCES: &[RowSpec] = &[
    RowSpec {
        label: "GitHub",
        helper: "Repositories, branches and pull requests.",
        body: RowBody::Planned(Planned::Action("Connect")),
    },
    RowSpec {
        label: "GitLab",
        helper: "Repositories, branches and merge requests.",
        body: RowBody::Planned(Planned::Action("Connect")),
    },
    RowSpec {
        label: "Linear",
        helper: "Lets an agent read and update Linear issues.",
        body: RowBody::Planned(Planned::Action("Connect")),
    },
    RowSpec {
        label: "Jira",
        helper: "Lets an agent read and update Jira issues.",
        body: RowBody::Planned(Planned::Action("Connect")),
    },
    RowSpec {
        label: "Default branch",
        helper: "The branch a new workspace starts from.",
        body: RowBody::Planned(Planned::Select(None)),
    },
    RowSpec {
        label: "Fetch on focus",
        helper: "Fetches the remote when a workspace gains focus.",
        body: RowBody::Planned(Planned::Switch),
    },
    RowSpec {
        label: "API budget",
        helper: "How many provider requests a workspace may make.",
        body: RowBody::Planned(Planned::Number {
            value: None,
            unit: "requests per hour",
        }),
    },
];

const HOSTS: &[RowSpec] = &[
    RowSpec {
        label: "This PC",
        helper: "The MassOrca service on this computer.",
        body: RowBody::ThisPc,
    },
    RowSpec {
        label: "Paired hosts",
        helper: "Remote hosts this PC is paired with.",
        body: RowBody::PairedHosts,
    },
    RowSpec {
        label: "Add host",
        helper: "Install MassOrca on a Linux host over SSH, and pair with it.",
        body: RowBody::AddHost,
    },
    RowSpec {
        label: "Paired phones",
        helper: "Pair a phone or another computer, and see the devices paired with this PC.",
        body: RowBody::PairingLink,
    },
];

const NOTIFICATIONS: &[RowSpec] = &[
    RowSpec {
        label: "Agent finished",
        helper: "Tells you when an agent ends a run you started.",
        body: RowBody::Planned(Planned::SwitchSelect),
    },
    RowSpec {
        label: "Waiting for you",
        helper: "Tells you when an agent needs an answer.",
        body: RowBody::Planned(Planned::SwitchSelect),
    },
    RowSpec {
        label: "Routine ran",
        helper: "Tells you when a scheduled routine runs.",
        body: RowBody::Planned(Planned::SwitchSelect),
    },
    RowSpec {
        label: "Extension notices",
        helper: "Tells you about updates and problems from extensions.",
        body: RowBody::Planned(Planned::SwitchSelect),
    },
    RowSpec {
        label: "Dictation",
        helper: "Turns speech into text in the composer.",
        body: RowBody::Planned(Planned::Switch),
    },
    RowSpec {
        label: "Speech model",
        helper: "The model that turns speech into text.",
        body: RowBody::Planned(Planned::Action("Download")),
    },
    RowSpec {
        label: "Transcription key",
        helper: "Stored in the secret vault, never shown in a field.",
        body: RowBody::Planned(Planned::Action("Add key")),
    },
];

const EXTENSIONS: &[RowSpec] = &[RowSpec {
    label: "Extensions",
    helper: "Managed in Marketplace, under Installed.",
    body: RowBody::Planned(Planned::Link),
}];

const PRIVACY: &[RowSpec] = &[
    RowSpec {
        label: "Crash reports",
        helper: "Off unless you turn it on. A preview comes before anything is sent.",
        body: RowBody::Planned(Planned::Switch),
    },
    RowSpec {
        label: "Telemetry",
        helper: "Usage statistics. Off unless you turn it on.",
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
        body: RowBody::Planned(Planned::Text(None)),
    },
    RowSpec {
        label: "Cache timers",
        helper: "How long cached data is kept on this PC.",
        body: RowBody::Planned(Planned::Select(None)),
    },
    RowSpec {
        label: "OS permissions",
        helper: "Folder and disk access that macOS asks for.",
        body: RowBody::Planned(Planned::Action("Review")),
    },
    RowSpec {
        label: "CLI registration",
        helper: "Makes the command-line tool available in terminals.",
        body: RowBody::Planned(Planned::Action("Register")),
    },
    RowSpec {
        label: "Experimental flags",
        helper: "Features still being built. Off unless you turn them on.",
        body: RowBody::Planned(Planned::Link),
    },
    RowSpec {
        label: "Advanced",
        helper: "Shows developer tools. Off by default.",
        body: RowBody::Planned(Planned::Switch),
    },
    RowSpec {
        label: "Developer tools",
        helper: "Tools for building and debugging MassOrca.",
        body: RowBody::Planned(Planned::Link),
    },
];

const ROUTING: &[RowSpec] = &[
    RowSpec {
        label: "Routing decisions",
        helper: "Why each Bot Teams request went to the bot it did.",
        body: RowBody::Planned(Planned::Status),
    },
    RowSpec {
        label: "Routing history",
        helper: "Removes every routing decision from this list.",
        body: RowBody::Planned(Planned::Action("Clear history")),
    },
];

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
        SettingsSection::Vault => &[],
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

    fn bodies(section: SettingsSection) -> Vec<RowBody> {
        rows(section).iter().map(|row| row.body).collect()
    }

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
            // The vault page draws its own header, search and scope cards instead of rows.
            if section != SettingsSection::Vault {
                assert!(!rows(section).is_empty(), "{section:?}");
            }
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
        assert!(theme.contains(&"Theme"));
        assert!(theme.contains(&"Import theme"));
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
        let density = search("density");
        assert_eq!(
            hit_line(&density, SettingsSection::Appearance),
            "1 match in this section"
        );
        assert_eq!(
            hit_line(&density, SettingsSection::General),
            "1 match in 1 section"
        );
        let spread = search("on");
        assert!(hit_line(&spread, SettingsSection::General).ends_with("sections"));
    }

    #[test]
    fn the_conversations_row_is_named_as_the_spec_names_it() {
        assert_eq!(
            rows(SettingsSection::General)[0].label,
            "Conversations without a project"
        );
    }

    #[test]
    fn the_hosts_list_starts_with_this_pc_then_marks_where_the_paired_hosts_go() {
        let hosts = rows(SettingsSection::Hosts);
        assert_eq!(hosts[0].body, RowBody::ThisPc);
        assert_eq!(hosts[1].body, RowBody::PairedHosts);
        assert_eq!(hosts[2].label, "Add host");
    }

    #[test]
    fn the_extensions_row_links_to_marketplace_installed() {
        assert_eq!(
            bodies(SettingsSection::Extensions),
            vec![RowBody::Planned(Planned::Link)]
        );
    }

    #[test]
    fn the_appearance_section_lists_the_spec_control_types() {
        let appearance = bodies(SettingsSection::Appearance);
        assert!(appearance.contains(&RowBody::Planned(Planned::Swatches)));
        assert!(appearance.contains(&RowBody::Planned(Planned::Slider {
            value: "100%",
            percent: 25,
        })));
        assert!(appearance.contains(&RowBody::Planned(Planned::SelectAction(
            None,
            "Save as preset"
        ))));
        assert!(appearance.contains(&RowBody::Planned(Planned::Checkboxes(&[
            "Agent status",
            "Memory",
            "Terminals",
            "Ports",
            "Hosts",
        ]))));
    }

    #[test]
    fn the_agents_section_has_no_fixed_list_of_agents() {
        let statuses = rows(SettingsSection::Agents)
            .iter()
            .filter(|row| row.body == RowBody::Planned(Planned::Status))
            .count();
        assert_eq!(
            statuses, 0,
            "installed agents come from the Marketplace, one row each"
        );
    }

    #[test]
    fn the_keyboard_table_has_its_columns() {
        assert!(
            bodies(SettingsSection::Input).contains(&RowBody::Planned(Planned::Table(&[
                "Action", "From", "Shortcut",
            ])))
        );
    }

    #[test]
    fn the_routing_section_offers_clear_history() {
        assert!(
            bodies(SettingsSection::Routing)
                .contains(&RowBody::Planned(Planned::Action("Clear history")))
        );
    }

    #[test]
    fn the_vault_page_says_sharing_is_a_link_and_has_no_status_card() {
        assert_eq!(
            SettingsSection::Vault.note(),
            Some(
                "Values never go into a chat, a prompt or a log. Sharing creates a link, not a copy: rotate the owner's value once and every linked scope uses it."
            )
        );
        assert!(
            rows(SettingsSection::Vault).is_empty(),
            "the vault page draws its own header and no Vault status card"
        );
    }

    #[test]
    fn the_sources_section_says_that_connecting_needs_the_providers_grant() {
        assert!(
            SettingsSection::Sources
                .note()
                .is_some_and(|note| note.contains("grant"))
        );
        assert_eq!(SettingsSection::General.note(), None);
    }
}
