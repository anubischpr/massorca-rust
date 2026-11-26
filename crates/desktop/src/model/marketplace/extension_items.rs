//! The Marketplace items of the service's extension packages (decision 0062). Every built-in catalogue entry is an
//! item. An installed package takes its catalogue entry's place with its installed state, and a package installed
//! from a folder or archive, which the catalogue does not list, is an item of its own.

use massorca_protocol::methods::extensions::{
    ExtensionItem, ExtensionReview, ExtensionState, Permission, RiskLevel, SourceKind,
};

use super::HostTarget;
use super::types::{ItemKind, ItemState, MarketplaceItem, MarketplaceItemId};

/// The first id an extension item takes. Agent kinds take the ids below it, one each.
const FIRST_ID: u32 = 1_000;

/// The publisher the built-in catalogue uses. Packages from anyone else are third-party.
const FIRST_PARTY: &str = "massorca";

/// The id each extension package keeps while this desktop runs, so an open page or a pending review keeps naming
/// the same item when the lists are read again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExtensionIds {
    packages: Vec<String>,
}

impl ExtensionIds {
    pub fn id_of(&mut self, package: &str) -> MarketplaceItemId {
        let index = match self.packages.iter().position(|known| known == package) {
            Some(index) => index,
            None => {
                self.packages.push(package.to_owned());
                self.packages.len() - 1
            }
        };
        MarketplaceItemId(FIRST_ID.saturating_add(u32::try_from(index).unwrap_or(u32::MAX)))
    }
}

/// The items of the extension packages: every catalogue entry, each installed package in its installed state in
/// place of its entry, and each installed package the catalogue does not list. An agent package whose agent kind the
/// service already lists is carried by that agent's item instead, so it is not listed twice.
/// `reviews` are the update checks: an installed package whose review differs from its installed sha256 offers that
/// review's version.
pub fn extension_items(
    installed: &[ExtensionItem],
    catalogue: &[ExtensionItem],
    agent_kind_ids: &[String],
    ids: &mut ExtensionIds,
    reviews: &[ExtensionReview],
) -> Vec<MarketplaceItem> {
    let in_catalogue = |id: &str| catalogue.iter().any(|entry| entry.id == id);
    let listed = catalogue.iter().map(|entry| {
        installed
            .iter()
            .find(|item| item.id == entry.id)
            .unwrap_or(entry)
    });
    let installed_only = installed
        .iter()
        .filter(|item| !in_catalogue(item.id.as_str()));
    listed
        .chain(installed_only)
        .filter(|entry| !is_listed_agent(entry, agent_kind_ids))
        .map(|entry| item(ids.id_of(&entry.id), entry, reviews))
        .collect()
}

/// Whether an agent package stands for an agent kind the service already lists.
fn is_listed_agent(entry: &ExtensionItem, agent_kind_ids: &[String]) -> bool {
    entry.listing.category == "agents"
        && entry
            .agents
            .iter()
            .any(|agent| agent_kind_ids.contains(&agent.id))
}

/// Whether a package is an agent package that provides this agent kind.
pub(super) fn provides_agent(entry: &ExtensionItem, kind: &str) -> bool {
    entry.listing.category == "agents" && entry.agents.iter().any(|agent| agent.id == kind)
}

fn item(
    id: MarketplaceItemId,
    entry: &ExtensionItem,
    reviews: &[ExtensionReview],
) -> MarketplaceItem {
    let developer = entry.listing.developer.as_ref().map_or_else(
        || entry.publisher.clone(),
        |developer| developer.name.clone(),
    );
    let kind = kind_of(&entry.listing.category);
    MarketplaceItem {
        id,
        kind,
        name: entry.name.clone(),
        publisher: entry.publisher.clone(),
        third_party: entry.publisher != FIRST_PARTY,
        version: entry.version.clone(),
        description: entry.description.clone(),
        adds: Vec::new(),
        state: state_of(entry.state, update_version(entry, reviews)),
        host: HostTarget::Local.id(),
        developer,
        category: kind.singular().to_owned(),
        permissions: permissions_text(&entry.permissions),
        found_on_this_pc: None,
        examples: banner_prompts(entry),
        agent_kind: agent_kind_of(entry),
        source_url: source_of(entry),
        previous_version: None,
        has_settings: !entry.settings.is_empty(),
        extension: Some(Box::new(entry.clone())),
    }
}

fn kind_of(category: &str) -> ItemKind {
    match category {
        "agents" => ItemKind::Agent,
        "skills" => ItemKind::Skill,
        "services" => ItemKind::Service,
        "themes" => ItemKind::Theme,
        _ => ItemKind::Extension,
    }
}

/// An installed package is on or off; an available one is not installed. A state from a newer service is not
/// installed, because this build cannot tell what it is.
pub(super) fn state_of(state: ExtensionState, update: Option<String>) -> ItemState {
    match state {
        ExtensionState::Enabled => ItemState::Installed {
            enabled: true,
            update,
        },
        ExtensionState::Disabled => ItemState::Installed {
            enabled: false,
            update,
        },
        ExtensionState::Available | ExtensionState::Unknown => ItemState::NotInstalled,
    }
}

/// The version a review offers an installed package: a review of the same package whose sha256 differs from the
/// installed one. An equal review is the installed package itself, so it offers nothing (decision 0062).
pub(super) fn update_version(entry: &ExtensionItem, reviews: &[ExtensionReview]) -> Option<String> {
    let installed = entry.installed.as_ref()?;
    reviews
        .iter()
        .find(|review| review.id == entry.id && review.sha256 != installed.sha256)
        .map(|review| review.version.clone())
}

/// The permissions as words, one per permission, or None when the package asks for none.
pub(super) fn permissions_text(permissions: &[Permission]) -> Option<String> {
    if permissions.is_empty() {
        return None;
    }
    let words: Vec<&str> = permissions
        .iter()
        .map(|permission| permission_word(&permission.kind))
        .collect();
    Some(words.join(", "))
}

pub(super) fn permission_word(kind: &str) -> &str {
    match kind {
        "storage" => "Storage",
        "notifications" => "Notifications",
        "workspaces:read" => "Workspaces (read)",
        "files:read" => "Files (read)",
        "files:write" => "Files (write)",
        "net" => "Network",
        "secrets" => "Secrets",
        "terminals:read" => "Terminals (read)",
        "terminals:write" => "Terminals (write)",
        "agents:read" => "Agents (read)",
        "agents:control" => "Agents (control)",
        "sessions:read" => "Sessions (read)",
        "process" => "Programs",
        "credentials" => "Agent credentials",
        other => other,
    }
}

/// Where a package was installed from, as a word.
pub fn source_word(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Catalogue => "Catalogue",
        SourceKind::Dir => "Folder",
        SourceKind::File => "Archive",
        SourceKind::Unknown => "Unknown source",
    }
}

/// The risk level as a word, for the Information part.
pub fn risk_word(level: RiskLevel) -> &'static str {
    match level {
        RiskLevel::Low => "Low",
        RiskLevel::Medium => "Medium",
        RiskLevel::High => "High",
        RiskLevel::Unknown => "Unknown",
    }
}

/// The banner's prompts: the caption of each mention example, which starts with the item's tag, such as "@claude".
pub(super) fn banner_prompts(entry: &ExtensionItem) -> Vec<String> {
    entry
        .listing
        .banner
        .iter()
        .flat_map(|banner| banner.examples.iter())
        .filter(|example| example.kind == "mention")
        .map(|example| example.caption.clone())
        .filter(|caption| !caption.trim().is_empty())
        .collect()
}

/// An agent package names the agent it provides, which the Connection part uses.
fn agent_kind_of(entry: &ExtensionItem) -> Option<String> {
    if entry.listing.category != "agents" {
        return None;
    }
    entry.agents.first().map(|agent| agent.id.clone())
}

/// Where the item comes from: an agent's vendor install page, else the developer's page; empty when neither is listed.
pub(super) fn source_of(entry: &ExtensionItem) -> String {
    entry
        .agents
        .iter()
        .find_map(|agent| agent.cli.as_ref().map(|cli| cli.source.clone()))
        .or_else(|| {
            entry
                .listing
                .developer
                .as_ref()
                .and_then(|developer| developer.url.clone())
        })
        .unwrap_or_default()
}

#[cfg(test)]
pub(super) mod tests {
    use massorca_protocol::methods::extensions::{
        Banner, BannerExample, Listing, Risk, UninstallEffects,
    };

    use super::*;

    pub(in crate::model::marketplace) fn package(
        id: &str,
        category: &str,
        state: ExtensionState,
    ) -> ExtensionItem {
        ExtensionItem {
            id: id.to_owned(),
            name: format!("Name of {id}"),
            description: "Keeps notes beside the workspace.".to_owned(),
            version: "1.0.0".to_owned(),
            publisher: "acme".to_owned(),
            state,
            installed: None,
            listing: Listing {
                category: category.to_owned(),
                ..crate::service::fixtures::base_listing()
            },
            permissions: Vec::new(),
            risk: Risk {
                ..crate::service::fixtures::base_risk()
            },
            readme: None,
            agents: Vec::new(),
            detected: None,
            settings: Vec::new(),
            uninstall: UninstallEffects {
                ..crate::service::fixtures::base_uninstall_effects()
            },
            problem: None,
        }
    }

    pub(in crate::model::marketplace) fn items(
        installed: &[ExtensionItem],
        catalogue: &[ExtensionItem],
        kinds: &[&str],
    ) -> Vec<MarketplaceItem> {
        let kinds: Vec<String> = kinds.iter().map(|kind| (*kind).to_owned()).collect();
        extension_items(
            installed,
            catalogue,
            &kinds,
            &mut ExtensionIds::default(),
            &[],
        )
    }

    #[test]
    fn an_installed_package_takes_its_catalogue_entry_with_its_state() {
        let entry = package("acme.notes", "extensions", ExtensionState::Available);
        let installed = package("acme.notes", "extensions", ExtensionState::Disabled);
        let found = items(
            std::slice::from_ref(&installed),
            std::slice::from_ref(&entry),
            &[],
        );
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].state,
            ItemState::Installed {
                enabled: false,
                update: None
            }
        );
        assert_eq!(found[0].kind, ItemKind::Extension);
        assert_eq!(
            found[0].extension.as_deref().map(|e| e.state),
            Some(ExtensionState::Disabled)
        );
    }

    #[test]
    fn an_available_package_is_not_installed_and_an_unknown_state_is_not_either() {
        let available = package("acme.a", "extensions", ExtensionState::Available);
        let unknown = package("acme.b", "extensions", ExtensionState::Unknown);
        let found = items(&[], &[available, unknown], &[]);
        assert!(
            found
                .iter()
                .all(|item| item.state == ItemState::NotInstalled)
        );
    }

    #[test]
    fn a_package_installed_from_a_folder_is_an_item_though_the_catalogue_lists_no_entry() {
        let installed = package("acme.folder", "extensions", ExtensionState::Enabled);
        let found = items(&[installed], &[], &[]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Name of acme.folder");
        assert!(found[0].state.is_installed());
        assert!(found[0].third_party);
    }

    #[test]
    fn an_agent_package_whose_kind_the_service_lists_is_not_listed_twice() {
        let mut agent = package("massorca.claude", "agents", ExtensionState::Available);
        agent.publisher = "massorca".to_owned();
        agent.agents = vec![massorca_protocol::methods::extensions::AgentItem {
            ..crate::service::fixtures::base_agent_item()
        }];
        let found = items(&[], std::slice::from_ref(&agent), &["claude"]);
        assert!(found.is_empty(), "the agent kind stands for the package");
        let listed = items(&[], std::slice::from_ref(&agent), &["codex"]);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].kind, ItemKind::Agent);
        assert_eq!(listed[0].agent_kind.as_deref(), Some("claude"));
        assert!(!listed[0].third_party);
    }

    #[test]
    fn permissions_and_mention_examples_come_from_the_package() {
        let mut entry = package("acme.notes", "extensions", ExtensionState::Available);
        entry.permissions = vec![
            Permission {
                kind: "storage".to_owned(),
                ..crate::service::fixtures::base_permission()
            },
            Permission {
                reason: "Reads them.".to_owned(),
                ..crate::service::fixtures::base_permission()
            },
        ];
        entry.listing.banner = Some(Banner {
            examples: vec![
                BannerExample {
                    text: Some("@notes".to_owned()),
                    ..crate::service::fixtures::base_banner_example()
                },
                BannerExample {
                    kind: "usage".to_owned(),
                    caption: "Other".to_owned(),
                    ..crate::service::fixtures::base_banner_example()
                },
            ],
            ..crate::service::fixtures::base_banner()
        });
        let found = items(&[], &[entry], &[]);
        assert_eq!(
            found[0].permissions.as_deref(),
            Some("Storage, Files (read)")
        );
        assert_eq!(found[0].examples, vec!["Ask it".to_owned()]);
    }

    #[test]
    fn ids_keep_each_package_while_the_desktop_runs() {
        let mut ids = ExtensionIds::default();
        let first = ids.id_of("acme.a");
        let second = ids.id_of("acme.b");
        assert_ne!(first, second);
        assert_eq!(ids.id_of("acme.a"), first);
        assert!(first.0 >= FIRST_ID);
    }

    #[test]
    fn the_danger_zone_names_what_removal_deletes_and_what_stays() {
        let mut entry = package("acme.notes", "extensions", ExtensionState::Enabled);
        entry.uninstall.storage = true;
        entry.uninstall.cli_binaries = vec!["claude".to_owned()];
        let found = items(&[], &[entry], &[]);
        let body = crate::model::marketplace::uninstall_body(&found[0]);
        assert_eq!(
            body,
            "Name of acme.notes is removed from MassOrca. Its package files are deleted. Its data is deleted. The claude CLI it installed stays on this computer."
        );
    }

    #[test]
    fn advanced_rows_name_the_install_source_and_checksum_once_installed() {
        use massorca_protocol::methods::extensions::{InstalledInfo, SourceKind, SourceRef};

        let mut entry = package("acme.notes", "extensions", ExtensionState::Enabled);
        entry.installed = Some(InstalledInfo {
            source: SourceRef {
                kind: SourceKind::Dir,
                reference: "notes".to_owned(),
            },
            ..crate::service::fixtures::base_installed_info()
        });
        let found = items(&[], &[entry], &[]);
        let rows = crate::model::marketplace::advanced_rows(&found[0]);
        assert!(
            rows.contains(&("Installed from", "Folder notes".to_owned())),
            "{rows:?}"
        );
        assert!(
            rows.contains(&("Checksum", "abc123".to_owned())),
            "{rows:?}"
        );
    }

    fn with_problem(code: &str, message: &str) -> ExtensionItem {
        let mut entry = package("acme.notes", "extensions", ExtensionState::Enabled);
        entry.problem = Some(massorca_protocol::methods::extensions::ItemProblem {
            code: code.to_owned(),
            message: message.to_owned(),
        });
        entry
    }

    #[test]
    fn a_tampered_package_is_a_danger_alert_with_a_plain_sentence() {
        let found = items(&[], &[with_problem("extension.tampered", "raw")], &[]);
        let alert = crate::model::marketplace::problem_alert(&found[0]).expect("an alert");
        assert_eq!(alert.tone, crate::model::marketplace::ProblemTone::Danger);
        assert!(
            alert.message.contains("changed on this computer"),
            "{}",
            alert.message
        );
        assert!(!alert.message.contains("raw"));
    }

    #[test]
    fn another_problem_is_a_warning_with_the_service_sentence() {
        let found = items(&[], &[with_problem("extension.io", "disk full")], &[]);
        let alert = crate::model::marketplace::problem_alert(&found[0]).expect("an alert");
        assert_eq!(alert.tone, crate::model::marketplace::ProblemTone::Warning);
        assert_eq!(alert.message, "disk full");
    }

    #[test]
    fn a_package_without_a_problem_has_no_alert_and_no_refusal() {
        let found = items(
            &[],
            &[package("acme.notes", "extensions", ExtensionState::Enabled)],
            &[],
        );
        assert_eq!(crate::model::marketplace::problem_alert(&found[0]), None);
        assert_eq!(
            crate::model::marketplace::package_refusal(
                &found[0],
                crate::model::marketplace::PackageAction::Enable
            ),
            None
        );
    }

    #[test]
    fn a_tampered_package_refuses_install_and_enable_but_not_uninstall_and_others_pass() {
        use crate::model::marketplace::{PackageAction, package_refusal};

        let tampered = items(&[], &[with_problem("extension.tampered", "raw")], &[]);
        let enable = package_refusal(&tampered[0], PackageAction::Enable).expect("refused");
        assert!(enable.contains("cannot be turned on"), "{enable}");
        let install = package_refusal(&tampered[0], PackageAction::Install).expect("refused");
        assert!(install.contains("cannot be installed again"), "{install}");

        let other = items(&[], &[with_problem("extension.io", "disk full")], &[]);
        assert_eq!(package_refusal(&other[0], PackageAction::Enable), None);
        assert_eq!(package_refusal(&other[0], PackageAction::Install), None);
    }

    #[test]
    fn the_information_part_names_the_risk_of_a_package() {
        let mut entry = package("acme.notes", "extensions", ExtensionState::Available);
        entry.risk.level = RiskLevel::High;
        let found = items(&[], &[entry], &[]);
        let rows = crate::model::marketplace::information(&found[0]);
        assert!(rows.contains(&("Risk", "High".to_owned())), "{rows:?}");
    }

    #[test]
    fn a_packages_links_reach_its_item_information() {
        let mut entry = package("acme.notes", "extensions", ExtensionState::Available);
        entry.listing.website = Some("https://acme.example".to_owned());
        entry.listing.terms = Some("https://acme.example/terms".to_owned());
        let found = items(&[], &[entry], &[]);
        assert_eq!(
            crate::model::marketplace::information_links(&found[0]),
            vec![
                ("Website", "https://acme.example".to_owned()),
                ("Terms of service", "https://acme.example/terms".to_owned()),
            ]
        );
    }

    #[test]
    fn a_service_lists_its_agent_kinds_and_its_packages_together() {
        use massorca_protocol::methods::agents::PackageState;
        use massorca_protocol::methods::extensions::AgentItem;

        use crate::model::marketplace::{MarketplaceSource, ServiceMarketplace};
        use crate::service::fixtures::agent_kind;

        let mut claude = package("massorca.claude", "agents", ExtensionState::Available);
        claude.agents = vec![AgentItem {
            ..crate::service::fixtures::base_agent_item()
        }];
        let notes = package("acme.notes", "extensions", ExtensionState::Enabled);
        let mut source = ServiceMarketplace::default();
        source.follow_agent_kinds(&[
            agent_kind("claude", "Claude", PackageState::Enabled),
            agent_kind("codex", "Codex", PackageState::Disabled),
        ]);
        source.follow_extensions(std::slice::from_ref(&notes), std::slice::from_ref(&claude));

        let snapshot = source.snapshot();
        let names: Vec<&str> = snapshot
            .items
            .iter()
            .map(|item| item.name.as_str())
            .collect();
        assert_eq!(names, ["Claude", "Codex", "Name of acme.notes"]);
        let notes_id = snapshot.items[2].id;
        assert!(notes_id.0 >= FIRST_ID);

        source.follow_extensions(std::slice::from_ref(&notes), &[]);
        assert_eq!(source.snapshot().items[2].id, notes_id);
    }

    /// The catalogue package of a built-in agent, which names the agent kind it provides.
    pub(in crate::model::marketplace) fn agent_package(
        kind: &str,
        state: ExtensionState,
    ) -> ExtensionItem {
        let mut entry = package(&format!("massorca.{kind}"), "agents", state);
        entry.publisher = FIRST_PARTY.to_owned();
        entry.agents = vec![massorca_protocol::methods::extensions::AgentItem {
            id: kind.to_owned(),
            ..crate::service::fixtures::base_agent_item()
        }];
        entry
    }

    #[test]
    fn each_built_in_agent_is_one_card_and_carries_its_package() {
        use massorca_protocol::methods::agents::{AgentKind, PackageState};

        use crate::model::marketplace::{MarketplaceSource, ServiceMarketplace};
        use crate::service::fixtures::agent_kind;

        let agents = [
            ("claude", "Claude Code"),
            ("codex", "Codex"),
            ("antigravity", "Antigravity"),
            ("gentle-shell", "Gentle Shell"),
        ];
        let kinds: Vec<AgentKind> = agents
            .iter()
            .map(|(id, name)| agent_kind(id, name, PackageState::Available))
            .collect();
        let catalogue: Vec<ExtensionItem> = agents
            .iter()
            .map(|(id, _)| agent_package(id, ExtensionState::Available))
            .collect();
        let mut source = ServiceMarketplace::default();
        source.follow_agent_kinds(&kinds);
        source.follow_extensions(&[], &catalogue);

        let snapshot = source.snapshot();
        let names: Vec<&str> = snapshot
            .items
            .iter()
            .map(|item| item.name.as_str())
            .collect();
        assert_eq!(
            names,
            ["Claude Code", "Codex", "Antigravity", "Gentle Shell"]
        );
        for (item, (id, _)) in snapshot.items.iter().zip(agents) {
            let package = item
                .extension
                .as_deref()
                .expect("the agent carries its package");
            assert_eq!(package.id, format!("massorca.{id}"));
            assert_eq!(item.state, ItemState::NotInstalled);
        }

        let installed = [agent_package("codex", ExtensionState::Disabled)];
        source.follow_extensions(&installed, &catalogue);
        let codex = source
            .snapshot()
            .items
            .into_iter()
            .find(|item| item.name == "Codex")
            .expect("the agent is listed");
        assert_eq!(
            codex.state,
            ItemState::Installed {
                enabled: false,
                update: None
            }
        );
        assert_eq!(
            codex.extension.as_deref().map(|entry| entry.state),
            Some(ExtensionState::Disabled)
        );
    }

    #[test]
    fn the_uninstall_offers_the_cli_only_when_the_package_installed_it() {
        use massorca_protocol::methods::extensions::{
            CliRecord, InstalledInfo, SourceKind, SourceRef,
        };

        use crate::model::marketplace::{
            CLI_REMOVAL_SENTENCE, offers_cli_removal, uninstall_body_for,
        };

        let record = |installed_by_package: bool| CliRecord {
            installed_by_package,
            ..crate::service::fixtures::base_cli_record()
        };
        let installed = |cli: Vec<CliRecord>| {
            let mut entry = package("acme.notes", "extensions", ExtensionState::Enabled);
            entry.uninstall.cli_binaries = vec!["claude".to_owned()];
            entry.installed = Some(InstalledInfo {
                source: SourceRef {
                    kind: SourceKind::Catalogue,
                    reference: "acme.notes".to_owned(),
                },
                cli,
                ..crate::service::fixtures::base_installed_info()
            });
            entry
        };

        let plain = items(&[], &[installed(Vec::new())], &[]);
        assert!(!offers_cli_removal(&plain[0]));
        let kept = items(&[], &[installed(vec![record(false)])], &[]);
        assert!(
            !offers_cli_removal(&kept[0]),
            "a CLI the package found stays out"
        );
        let offered = items(&[], &[installed(vec![record(true)])], &[]);
        assert!(offers_cli_removal(&offered[0]));

        assert!(
            uninstall_body_for(&offered[0], false).contains("The claude CLI it installed stays"),
            "unticked, the body says the CLI stays"
        );
        assert!(
            !uninstall_body_for(&offered[0], true).contains("stays on this computer"),
            "ticked, the body no longer says the CLI stays"
        );
        assert_eq!(
            CLI_REMOVAL_SENTENCE,
            "This removes the CLI from this computer, so it stops working outside MassOrca too."
        );
    }
}
