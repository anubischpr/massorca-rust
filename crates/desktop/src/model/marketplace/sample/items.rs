//! The placeholder catalogue of the sample Marketplace: two agents, five extensions and a skill.

use super::super::types::{HostId, ItemKind, ItemState, MarketplaceItem, MarketplaceItemId};

/// The sample catalogue in id order: two agents, five extensions and a skill (placeholder data).
pub(super) fn sample_items() -> Vec<MarketplaceItem> {
    vec![
        MarketplaceItem {
            state: ItemState::Installed {
                enabled: true,
                update: None,
            },
            agent_kind: Some("claude".to_owned()),
            has_settings: true,
            examples: strings(&[
                "@Claude review the open changes on this branch",
                "@Claude explain the failing test in this project",
                "@Claude summarise today's sessions",
            ]),
            adds: strings(&["Agent in the model picker"]),
            description: "Starts Claude sessions from MassOrca and lets you pick Claude in the model picker."
                .to_owned(),
            ..sample_item(1, ItemKind::Agent, "Claude", "MassOrca", "1.0.0", "Agent")
        },
        MarketplaceItem {
            state: ItemState::Installed {
                enabled: true,
                update: None,
            },
            host: HostId(2),
            agent_kind: Some("codex".to_owned()),
            examples: strings(&[
                "@Codex explain the last build log",
                "@Codex suggest a name for this branch",
                "@Codex list the files changed today",
            ]),
            adds: strings(&["Agent in the model picker"]),
            description: "Starts Codex sessions from MassOrca and lets you pick Codex in the model picker."
                .to_owned(),
            ..sample_item(2, ItemKind::Agent, "Codex", "MassOrca extras", "1.0.0", "Agent")
        },
        MarketplaceItem {
            third_party: true,
            developer: "Example developer".to_owned(),
            permissions: Some(
                "Reads repository data; network access to api.github.com".to_owned(),
            ),
            found_on_this_pc: Some("GitHub CLI 2.60 (detected)".to_owned()),
            examples: strings(&[
                "@GitHub summarise the open pull requests in this repository",
                "@GitHub draft review comments for the open pull request",
                "@GitHub list the issues assigned to me",
            ]),
            adds: strings(&["3 commands", "Sidebar section"]),
            description: "Reads pull requests and issues from GitHub and drafts review comments."
                .to_owned(),
            ..sample_item(
                3,
                ItemKind::Extension,
                "GitHub",
                "Third party",
                "1.3.0",
                "Source control",
            )
        },
        MarketplaceItem {
            third_party: true,
            state: ItemState::Incompatible {
                requires: "MassOrca 1.6 or later".to_owned(),
            },
            adds: strings(&["Work panel tab"]),
            description: "Follows a log file in a Work panel tab, with a filter for errors."
                .to_owned(),
            ..sample_item(
                4,
                ItemKind::Extension,
                "Log tail",
                "Third party",
                "0.2.0",
                "Developer tools",
            )
        },
        MarketplaceItem {
            third_party: true,
            state: ItemState::Installed {
                enabled: true,
                update: Some("2.1.0".to_owned()),
            },
            adds: strings(&["Rail item"]),
            previous_version: Some("1.9.0".to_owned()),
            description: "Shows usage for each model in a rail item, with a detail popover on hover."
                .to_owned(),
            ..sample_item(
                5,
                ItemKind::Extension,
                "Model usage panel",
                "Third party",
                "2.0.0",
                "Usage",
            )
        },
        MarketplaceItem {
            state: ItemState::Installed {
                enabled: true,
                update: None,
            },
            adds: strings(&["Project sidebar tab", "Status item"]),
            previous_version: Some("0.3.0".to_owned()),
            description: "Short notes per project and branch, shown in a Project sidebar tab with a count in the status bar."
                .to_owned(),
            ..sample_item(
                6,
                ItemKind::Extension,
                "Quick notes",
                "MassOrca extras",
                "0.4.1",
                "Productivity",
            )
        },
        MarketplaceItem {
            examples: strings(&[
                "@Release notes write notes for this branch",
                "@Release notes group the merged pull requests by area",
                "@Release notes draft the notes for the next tag",
            ]),
            adds: strings(&["Command"]),
            description: "Writes release notes for a branch from its merged pull requests, in the words of each pull request title."
                .to_owned(),
            ..sample_item(
                7,
                ItemKind::Skill,
                "Release notes",
                "MassOrca extras",
                "1.0.1",
                "Writing",
            )
        },
        MarketplaceItem {
            state: ItemState::Installed {
                enabled: true,
                update: None,
            },
            adds: strings(&["Sidebar section", "2 commands"]),
            description: "Shows your workspaces as a board, grouped by status: open, running, waiting for you and done."
                .to_owned(),
            ..sample_item(
                8,
                ItemKind::Extension,
                "Workspace board",
                "MassOrca extras",
                "1.2.0",
                "Workspaces",
            )
        },
    ]
}

/// An item with the sample's common fields. Each entry overrides what differs.
fn sample_item(
    id: u32,
    kind: ItemKind,
    name: &str,
    publisher: &str,
    version: &str,
    category: &str,
) -> MarketplaceItem {
    let slug = name.to_lowercase().replace(' ', "-");
    MarketplaceItem {
        id: MarketplaceItemId(id),
        kind,
        name: name.to_owned(),
        publisher: publisher.to_owned(),
        third_party: false,
        version: version.to_owned(),
        description: String::new(),
        adds: Vec::new(),
        state: ItemState::NotInstalled,
        host: HostId(1),
        developer: publisher.to_owned(),
        category: category.to_owned(),
        permissions: None,
        found_on_this_pc: None,
        examples: Vec::new(),
        agent_kind: None,
        source_url: format!("https://example.com/marketplace/{slug}"),
        previous_version: None,
        has_settings: false,
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}
