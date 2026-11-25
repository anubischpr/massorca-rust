//! The nine parts of a Marketplace item page in the owner's order (teams-extensions-settings.md 7.2,
//! decision 0059): the parts in order, why a part does not apply to an item, and the rows each part shows.

use massorca_protocol::methods::extensions::ExtensionItem;

use super::extension_items::{risk_word, source_word};
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

/// Every part of the page, in order. A part that does not apply to an item stays in its place and says why.
pub const PARTS: [Part; 9] = [
    Part::Logo,
    Part::Name,
    Part::Host,
    Part::Banner,
    Part::Connection,
    Part::Information,
    Part::Settings,
    Part::AdvancedOptions,
    Part::DangerZone,
];

/// Why a part does not apply to the item, in one sentence; None when it applies. The banner has no sentence: with
/// no example prompts it is left out of the page.
pub fn absent_reason(item: &MarketplaceItem, part: Part) -> Option<&'static str> {
    match part {
        Part::Connection if item.agent_kind.is_none() => {
            Some("Only agents connect to accounts, so this item has no connection.")
        }
        Part::AdvancedOptions if !item.state.is_installed() => {
            Some("Advanced options appear once the item is installed on this computer.")
        }
        _ => None,
    }
}

/// The tag a banner's prompts start with: the item's mention, such as "@claude", or else its name.
pub fn banner_tag(mention: Option<&str>, name: &str) -> String {
    match mention {
        Some(mention) => format!("@{mention}"),
        None => format!("@{name}"),
    }
}

/// How many palette colours (`banner.palette.1` to `.8`) a banner without a listed colour picks from.
pub const BANNER_PALETTE_SLOTS: usize = 8;

/// The palette slot, 1 to 8, of a banner whose listing gives no colour: a stable hash of the package id, or of the
/// name when there is no package.
pub fn banner_palette_slot(item: &MarketplaceItem) -> usize {
    let seed = item
        .extension
        .as_deref()
        .map_or(item.name.as_str(), |entry| entry.id.as_str());
    palette_slot_of(seed)
}

/// FNV-1a of the seed, modulo the slot count, plus one; unlike the std hasher it gives the same slot on every run.
fn palette_slot_of(seed: &str) -> usize {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    let hash = seed.bytes().fold(OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    });
    usize::try_from(hash % BANNER_PALETTE_SLOTS as u64).unwrap_or_default() + 1
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

/// The links of the Information part: the website, privacy policy and terms the package names, each only when present.
pub fn information_links(item: &MarketplaceItem) -> Vec<(&'static str, String)> {
    let Some(extension) = item.extension.as_deref() else {
        return Vec::new();
    };
    [
        ("Website", extension.listing.website.clone()),
        ("Privacy policy", extension.listing.privacy.clone()),
        ("Terms of service", extension.listing.terms.clone()),
    ]
    .into_iter()
    .filter_map(|(label, url)| url.map(|url| (label, url)))
    .collect()
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

/// The body of the uninstall confirm. An agent's accounts stay on this computer until the person
/// removes them (decision 0059). A package names what its removal deletes, and what it leaves in place.
pub fn uninstall_body(item: &MarketplaceItem) -> String {
    uninstall_body_for(item, false)
}

/// The body of the uninstall confirm when `remove_cli` says the CLI the package installed is removed too (ticked).
pub fn uninstall_body_for(item: &MarketplaceItem, remove_cli: bool) -> String {
    if let Some(extension) = item.extension.as_deref() {
        return package_uninstall_body(&item.name, extension, remove_cli);
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

/// The sentence the uninstall confirm shows under its CLI removal, when the package installed the CLI.
pub const CLI_REMOVAL_SENTENCE: &str =
    "This removes the CLI from this computer, so it stops working outside MassOrca too.";

/// Whether the uninstall confirm offers to remove a CLI the package installed: its install record names one.
pub fn offers_cli_removal(item: &MarketplaceItem) -> bool {
    item.extension
        .as_deref()
        .and_then(|extension| extension.installed.as_ref())
        .is_some_and(|installed| installed.cli.iter().any(|cli| cli.installed_by_package))
}

/// What removing an extension package deletes, and what it leaves on this computer, in the service's own terms.
fn package_uninstall_body(name: &str, extension: &ExtensionItem, remove_cli: bool) -> String {
    let effects = &extension.uninstall;
    let mut sentences = vec![format!("{name} is removed from MassOrca.")];
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
    if !remove_cli {
        for binary in &effects.cli_binaries {
            sentences.push(format!(
                "The {binary} CLI it installed stays on this computer."
            ));
        }
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
    fn marketplace_banner_palette_slot_is_pinned_for_a_package_id() {
        assert_eq!(
            palette_slot_of("acme.notes"),
            1,
            "FNV-1a of the id, modulo 8, plus 1"
        );
        assert_eq!(palette_slot_of("massorca.codex"), 6);
    }

    #[test]
    fn marketplace_banner_palette_slot_spreads_items_over_all_eight_colours() {
        let slots: std::collections::BTreeSet<usize> = (0..24)
            .map(|index| palette_slot_of(&format!("acme.item-{index}")))
            .collect();
        assert_eq!(
            slots.iter().copied().collect::<Vec<_>>(),
            (1..=8).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_page_has_the_nine_parts_in_the_owners_order() {
        assert_eq!(
            PARTS,
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
    fn the_banner_has_no_sentence_when_an_item_has_no_examples() {
        let item = find(&sample(), 8);
        assert!(
            item.examples.is_empty(),
            "the sample item lists no examples"
        );
        assert_eq!(
            absent_reason(&item, Part::Banner),
            None,
            "the banner is left out, with no sentence"
        );
    }

    #[test]
    fn the_banner_tag_is_the_mention_or_the_name() {
        assert_eq!(banner_tag(Some("claude"), "Claude"), "@claude");
        assert_eq!(banner_tag(None, "Notes"), "@Notes");
    }

    #[test]
    fn the_connection_says_why_only_an_agent_has_one() {
        let snapshot = sample();
        assert_eq!(absent_reason(&find(&snapshot, 1), Part::Connection), None);
        assert_eq!(
            absent_reason(&find(&snapshot, 3), Part::Connection),
            Some("Only agents connect to accounts, so this item has no connection.")
        );
    }

    #[test]
    fn advanced_options_say_why_an_item_not_installed_has_none() {
        let snapshot = sample();
        assert_eq!(
            absent_reason(&find(&snapshot, 5), Part::AdvancedOptions),
            None
        );
        assert_eq!(
            absent_reason(&find(&snapshot, 3), Part::AdvancedOptions),
            Some("Advanced options appear once the item is installed on this computer.")
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
