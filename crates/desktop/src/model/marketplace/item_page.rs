//! The nine parts of a Marketplace item page in the owner's order (teams-extensions-settings.md 7.2,
//! decision 0059): which parts an item has, and the rows each part shows.

use massorca_protocol::methods::extensions::{ExtensionItem, SourceKind};

use super::extension_items::risk_word;
use super::types::{AccountState, ItemState, MarketplaceItem, MarketplaceSnapshot};

/// One part of the item page, from the top down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Logo,
    Name,
    Host,
    Banner,
    Connection,
    Information,
    Settings,
    AdvancedOptions,
    DangerZone,
}

impl Part {
    /// The heading of the part; the logo, name and host have a heading of their own.
    pub fn title(self) -> &'static str {
        match self {
            Part::Logo => "Logo",
            Part::Name => "Name",
            Part::Host => "Host",
            Part::Banner => "Invocation",
            Part::Connection => "Connection",
            Part::Information => "Information",
            Part::Settings => "Settings",
            Part::AdvancedOptions => "Advanced Options",
            Part::DangerZone => "Danger Zone",
        }
    }
}

/// The parts an item shows, in order. Banner needs examples, Connection needs an agent and
/// Advanced Options needs an installed item; the other parts always show.
pub fn parts(item: &MarketplaceItem) -> Vec<Part> {
    let mut parts = vec![Part::Logo, Part::Name, Part::Host];
    if !item.examples.is_empty() {
        parts.push(Part::Banner);
    }
    if item.agent_kind.is_some() {
        parts.push(Part::Connection);
    }
    parts.push(Part::Information);
    parts.push(Part::Settings);
    if item.state.is_installed() {
        parts.push(Part::AdvancedOptions);
    }
    parts.push(Part::DangerZone);
    parts
}

/// The state chip beside the name. An installed agent shows its active account, and "Connected"
/// for a signed-in one; everything else shows the item's own state.
pub fn state_chip(item: &MarketplaceItem, snapshot: &MarketplaceSnapshot) -> &'static str {
    if let (Some(kind), true) = (&item.agent_kind, item.state.is_installed()) {
        let active = snapshot
            .accounts_for(kind)
            .into_iter()
            .find(|account| account.active);
        return match active {
            Some(account) if account.state == AccountState::SignedIn => "Connected",
            Some(account) => account.state.word(),
            None => "Not set up",
        };
    }
    match &item.state {
        ItemState::NotInstalled => "Not installed",
        ItemState::Installed { enabled: false, .. } => "Disabled",
        ItemState::Installed {
            update: Some(_), ..
        } => "Update available",
        ItemState::Installed { .. } => "Installed",
        ItemState::Installing { .. } => "Installing",
        ItemState::Failed { .. } => "Failed to start",
        ItemState::Incompatible { .. } => "Incompatible",
    }
}

/// The rows of the Information part. A tool found on this computer is information only: it never
/// makes the item installed.
pub fn information(item: &MarketplaceItem) -> Vec<(&'static str, String)> {
    let mut rows = vec![
        ("Developer", item.developer.clone()),
        ("Category", item.category.clone()),
    ];
    if let Some(permissions) = &item.permissions {
        rows.push(("Permissions", permissions.clone()));
    }
    if let Some(extension) = &item.extension {
        rows.push(("Risk", risk_word(extension.risk.level).to_owned()));
    }
    if let Some(found) = &item.found_on_this_pc {
        rows.push(("Found on this PC", found.clone()));
    }
    rows
}

/// The rows of the Settings part. They exist only once the item is installed; an agent's default
/// agent needs a setting the service does not have yet.
pub fn settings_rows(item: &MarketplaceItem) -> Vec<(&'static str, &'static str)> {
    if !item.state.is_installed() {
        return Vec::new();
    }
    if item.agent_kind.is_some() {
        return vec![(
            "Default agent",
            "Starts a new chat with this agent. Not available yet: the service has no default agent setting.",
        )];
    }
    if item.has_settings {
        return vec![(
            "Settings",
            "Not available yet: this item's settings need the service.",
        )];
    }
    Vec::new()
}

/// The rows of Advanced Options: the versions, the source and who can use the item.
pub fn advanced_rows(item: &MarketplaceItem) -> Vec<(&'static str, String)> {
    let mut rows = vec![("Version", item.version.clone())];
    if let Some(previous) = &item.previous_version {
        rows.push(("Previous version", previous.clone()));
    }
    rows.push(("Source", item.source_url.clone()));
    if let Some(installed) = item.extension.as_deref().and_then(|e| e.installed.as_ref()) {
        rows.push((
            "Installed from",
            format!(
                "{} {}",
                source_word(installed.source.kind),
                installed.source.reference
            ),
        ));
        rows.push(("Checksum", installed.sha256.clone()));
    }
    rows.push(("Who can use it", "Only you".to_owned()));
    rows
}

fn source_word(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Catalogue => "Catalogue",
        SourceKind::Dir => "Folder",
        SourceKind::File => "Archive",
        SourceKind::Unknown => "Unknown source",
    }
}

/// The body of the uninstall confirm. An agent's accounts stay on this computer until the person
/// removes them (decision 0059). A package names what its removal deletes, and what it leaves in place.
pub fn uninstall_body(item: &MarketplaceItem) -> String {
    if let Some(extension) = item.extension.as_deref() {
        return package_uninstall_body(&item.name, extension);
    }
    if item.agent_kind.is_some() {
        format!(
            "{} stops starting new chats. Its accounts stay on this computer until you remove them.",
            item.name
        )
    } else {
        format!(
            "{} is removed from this computer, and its settings are deleted.",
            item.name
        )
    }
}

/// What removing an extension package deletes, and what it leaves on this computer, in the service's own terms.
fn package_uninstall_body(name: &str, extension: &ExtensionItem) -> String {
    let effects = &extension.uninstall;
    let mut sentences = vec![format!("{name} is removed from this computer.")];
    if effects.package_files {
        sentences.push("Its package files are deleted.".to_owned());
    }
    if effects.storage {
        sentences.push("Its data is deleted.".to_owned());
    }
    if effects.secrets_unscoped {
        sentences.push("Every secret it stored is deleted.".to_owned());
    } else if !effects.secret_names.is_empty() {
        sentences.push(format!(
            "Its secrets are deleted: {}.",
            effects.secret_names.join(", ")
        ));
    }
    if !effects.connections.is_empty() {
        sentences.push("Its connections are removed.".to_owned());
    }
    for binary in &effects.cli_binaries {
        sentences.push(format!(
            "The {binary} CLI it installed stays on this computer."
        ));
    }
    sentences.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::marketplace::{
        MarketplaceItemId, MarketplaceSource, SampleMarketplaceSource,
    };

    fn sample() -> MarketplaceSnapshot {
        SampleMarketplaceSource::default().snapshot()
    }

    fn find(snapshot: &MarketplaceSnapshot, id: u32) -> MarketplaceItem {
        snapshot
            .item(MarketplaceItemId(id))
            .expect("the sample has the item")
            .clone()
    }

    #[test]
    fn an_installed_agent_page_shows_the_nine_parts_in_the_owners_order() {
        let claude = find(&sample(), 1);
        assert_eq!(
            parts(&claude),
            [
                Part::Logo,
                Part::Name,
                Part::Host,
                Part::Banner,
                Part::Connection,
                Part::Information,
                Part::Settings,
                Part::AdvancedOptions,
                Part::DangerZone,
            ]
        );
    }

    #[test]
    fn parts_follow_what_the_item_has() {
        let snapshot = sample();
        assert_eq!(
            parts(&find(&snapshot, 3)),
            [
                Part::Logo,
                Part::Name,
                Part::Host,
                Part::Banner,
                Part::Information,
                Part::Settings,
                Part::DangerZone,
            ],
            "a plugin that is not installed has examples but no connection or advanced options"
        );
        assert_eq!(
            parts(&find(&snapshot, 8)),
            [
                Part::Logo,
                Part::Name,
                Part::Host,
                Part::Information,
                Part::Settings,
                Part::AdvancedOptions,
                Part::DangerZone,
            ]
        );
    }

    #[test]
    fn the_chip_reads_the_active_account_for_an_installed_agent() {
        let snapshot = sample();
        assert_eq!(state_chip(&find(&snapshot, 1), &snapshot), "Connected");
        assert_eq!(state_chip(&find(&snapshot, 2), &snapshot), "Not set up");
        assert_eq!(state_chip(&find(&snapshot, 3), &snapshot), "Not installed");
        assert_eq!(state_chip(&find(&snapshot, 4), &snapshot), "Incompatible");
        assert_eq!(
            state_chip(&find(&snapshot, 5), &snapshot),
            "Update available"
        );
    }

    #[test]
    fn a_found_tool_is_information_and_settings_appear_only_after_install() {
        let snapshot = sample();
        let github = find(&snapshot, 3);
        assert!(
            information(&github)
                .contains(&("Found on this PC", "GitHub CLI 2.60 (detected)".to_owned()))
        );
        assert!(settings_rows(&github).is_empty(), "not installed yet");
        assert_eq!(settings_rows(&find(&snapshot, 1))[0].0, "Default agent");
    }

    #[test]
    fn advanced_rows_name_the_versions_and_who_can_use_the_item() {
        let rows = advanced_rows(&find(&sample(), 5));
        assert_eq!(rows[0], ("Version", "2.0.0".to_owned()));
        assert!(rows.contains(&("Previous version", "1.9.0".to_owned())));
        assert_eq!(
            rows.last(),
            Some(&("Who can use it", "Only you".to_owned()))
        );
    }

    #[test]
    fn an_agent_uninstall_says_its_accounts_stay() {
        assert!(uninstall_body(&find(&sample(), 1)).contains("accounts stay"));
    }
}
