//! The Marketplace of a connected service: its agent kinds, as `agent.kinds` lists them, and its extension packages
//! (decision 0062). Agent kinds stay as they are; extension items are added beside them.

use std::collections::BTreeMap;

use massorca_protocol::methods::agents::{AccountSummary, AgentKind, PackageState};
use massorca_protocol::methods::extensions::ExtensionItem;

use super::service_accounts::ServiceAccounts;

use super::extension_items::{ExtensionIds, extension_items};
use super::source::{MarketplaceSource, SourceError};
use super::types::{
    Host, HostId, HostState, ItemKind, ItemState, MarketplaceItem, MarketplaceItemId,
    MarketplaceSnapshot,
};
use crate::service::Refusal;

/// The host every agent item is on: the service the desktop is connected to.
pub(super) const SERVICE_HOST: HostId = HostId(1);

/// The Marketplace a connected service makes up. It is empty until a connect reads its lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceMarketplace {
    snapshot: MarketplaceSnapshot,
    kinds: Vec<AgentKind>,
    installed: Vec<ExtensionItem>,
    catalogue: Vec<ExtensionItem>,
    ids: ExtensionIds,
    /// The accounts of every kind, as the service lists them.
    accounts: ServiceAccounts,
}

impl ServiceMarketplace {
    /// Builds the snapshot from the lists it holds: the agent items first, then the extension items.
    fn rebuild(&mut self) {
        let base = agent_snapshot(&self.kinds);
        let kind_ids: Vec<String> = self.kinds.iter().map(|kind| kind.id.clone()).collect();
        let extensions =
            extension_items(&self.installed, &self.catalogue, &kind_ids, &mut self.ids);
        let items = base.items.into_iter().chain(extensions).collect();
        self.snapshot = MarketplaceSnapshot { items, ..base };
    }
}

/// The Marketplace snapshot for the agent kinds a service lists, in the order it lists them.
pub fn agent_snapshot(kinds: &[AgentKind]) -> MarketplaceSnapshot {
    MarketplaceSnapshot {
        items: kinds
            .iter()
            .enumerate()
            .map(|(index, kind)| agent_item(index, kind))
            .collect(),
        hosts: vec![Host {
            id: SERVICE_HOST,
            name: "MassOrca service".to_owned(),
            state: HostState::Connected,
        }],
        accounts: Vec::new(),
        sign_in: None,
        default_agent_available: false,
    }
}

/// One agent kind as a Marketplace item. Its install state is the package's; `detected` only says the
/// program was found on this computer, which is information rather than an install.
fn agent_item(index: usize, kind: &AgentKind) -> MarketplaceItem {
    // The service's own kinds have owner `core`; a profile's file reports `user`.
    let publisher = match kind.owner.as_str() {
        "core" => "MassOrca",
        "user" => "This profile",
        _ => "Extension",
    };
    MarketplaceItem {
        id: MarketplaceItemId(u32::try_from(index + 1).unwrap_or(u32::MAX)),
        kind: ItemKind::Agent,
        name: kind.name.clone(),
        publisher: publisher.to_owned(),
        third_party: kind.owner != "core",
        version: kind.package.version.clone().unwrap_or_default(),
        description: format!(
            "Starts {} sessions from MassOrca and lets you pick it in the model picker.",
            kind.name
        ),
        adds: vec!["Agent in the model picker".to_owned()],
        state: state_of(kind),
        host: SERVICE_HOST,
        developer: publisher.to_owned(),
        category: "Agent".to_owned(),
        permissions: None,
        found_on_this_pc: kind.detected.then(|| format!("{} (detected)", kind.name)),
        examples: Vec::new(),
        agent_kind: Some(kind.id.clone()),
        source_url: String::new(),
        previous_version: None,
        has_settings: false,
        extension: None,
    }
}

/// A kind's package as its item state: an installed package is on or off, an available one is not installed.
fn state_of(kind: &AgentKind) -> ItemState {
    match kind.package.state {
        PackageState::Enabled => ItemState::Installed {
            enabled: true,
            update: None,
        },
        PackageState::Disabled => ItemState::Installed {
            enabled: false,
            update: None,
        },
        PackageState::Available | PackageState::Unknown => ItemState::NotInstalled,
    }
}

impl MarketplaceSource for ServiceMarketplace {
    fn snapshot(&self) -> MarketplaceSnapshot {
        let mut snapshot = self.snapshot.clone();
        snapshot.accounts = self.accounts.rows().to_vec();
        snapshot
    }

    fn follow_accounts(
        &mut self,
        accounts: &BTreeMap<String, Result<Vec<AccountSummary>, Refusal>>,
    ) {
        self.accounts.follow(accounts);
    }

    fn service_account(&self, account: super::types::AccountId) -> Option<(String, String)> {
        self.accounts.key(account)
    }

    fn account_number(&mut self, kind: &str, account: &str) -> Option<super::types::AccountId> {
        Some(self.accounts.number(kind, account))
    }

    fn follow_agent_kinds(&mut self, kinds: &[AgentKind]) {
        self.kinds = kinds.to_vec();
        self.rebuild();
    }

    fn follow_extensions(&mut self, installed: &[ExtensionItem], catalogue: &[ExtensionItem]) {
        self.installed = installed.to_vec();
        self.catalogue = catalogue.to_vec();
        self.rebuild();
    }

    fn set_enabled(&mut self, _: MarketplaceItemId, _: bool) -> Result<(), SourceError> {
        Err(agents_unchangeable())
    }

    fn uninstall(&mut self, _: MarketplaceItemId) -> Result<(), SourceError> {
        Err(agents_unchangeable())
    }

    fn add_account(&mut self, _: &str) -> Result<(), SourceError> {
        Err(accounts_unavailable())
    }

    fn open_sign_in_page(&mut self) -> Result<(), SourceError> {
        Err(accounts_unavailable())
    }

    fn submit_sign_in_code(&mut self, _: &str) -> Result<(), SourceError> {
        Err(accounts_unavailable())
    }

    fn retry_sign_in(&mut self) -> Result<(), SourceError> {
        Err(accounts_unavailable())
    }

    fn cancel_sign_in(&mut self) {}

    fn activate_account(&mut self, _: super::types::AccountId) -> Result<(), SourceError> {
        Err(accounts_unavailable())
    }

    fn remove_account(&mut self, _: super::types::AccountId) -> Result<(), SourceError> {
        Err(accounts_unavailable())
    }

    fn check_account(&mut self, _: super::types::AccountId) -> Result<(), SourceError> {
        Err(accounts_unavailable())
    }
}

fn agents_unchangeable() -> SourceError {
    SourceError::Unavailable("Agents cannot be changed from the Marketplace yet.".to_owned())
}

fn accounts_unavailable() -> SourceError {
    SourceError::Unavailable("Agent accounts are not available from the service yet.".to_owned())
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::agents::PackageState;

    use super::*;
    use crate::model::marketplace::{AgentLink, ItemKind, ItemState, installed_agent_links};
    use crate::service::fixtures::agent_kind;

    /// Claude is installed and on, Codex installed but off, and Cursor only listed; Cursor's program is found.
    fn service_kinds() -> Vec<AgentKind> {
        vec![
            agent_kind("claude", "Claude", PackageState::Enabled),
            agent_kind("codex", "Codex", PackageState::Disabled),
            agent_kind("cursor", "Cursor", PackageState::Available),
        ]
    }

    fn item_named<'a>(
        snapshot: &'a MarketplaceSnapshot,
        name: &str,
    ) -> &'a super::super::types::MarketplaceItem {
        snapshot
            .items
            .iter()
            .find(|item| item.name == name)
            .expect("the item is listed")
    }

    #[test]
    fn each_kind_the_service_lists_is_an_agent_item_under_its_id() {
        let snapshot = agent_snapshot(&service_kinds());
        let listed: Vec<(&str, ItemKind, Option<&str>)> = snapshot
            .items
            .iter()
            .map(|item| (item.name.as_str(), item.kind, item.agent_kind.as_deref()))
            .collect();
        assert_eq!(
            listed,
            [
                ("Claude", ItemKind::Agent, Some("claude")),
                ("Codex", ItemKind::Agent, Some("codex")),
                ("Cursor", ItemKind::Agent, Some("cursor")),
            ]
        );
    }

    #[test]
    fn an_enabled_or_disabled_package_is_installed_and_an_available_one_is_not() {
        let snapshot = agent_snapshot(&service_kinds());
        assert_eq!(
            item_named(&snapshot, "Claude").state,
            ItemState::Installed {
                enabled: true,
                update: None
            }
        );
        assert_eq!(
            item_named(&snapshot, "Codex").state,
            ItemState::Installed {
                enabled: false,
                update: None
            }
        );
        assert_eq!(
            item_named(&snapshot, "Cursor").state,
            ItemState::NotInstalled
        );
    }

    #[test]
    fn found_on_this_pc_is_information_and_never_makes_a_kind_installed() {
        let snapshot = agent_snapshot(&service_kinds());
        let cursor = item_named(&snapshot, "Cursor");
        assert!(cursor.found_on_this_pc.is_some(), "the program is found");
        assert_eq!(cursor.state, ItemState::NotInstalled);
    }

    #[test]
    fn settings_lists_only_the_installed_kinds_by_name() {
        let snapshot = agent_snapshot(&service_kinds());
        let names: Vec<String> = installed_agent_links(&snapshot)
            .into_iter()
            .map(|link: AgentLink| link.name)
            .collect();
        assert_eq!(names, ["Claude", "Codex"]);
    }

    #[test]
    fn a_service_that_lists_no_kinds_lists_no_items() {
        let snapshot = agent_snapshot(&[]);
        assert!(snapshot.items.is_empty());
    }

    #[test]
    fn following_the_kinds_again_replaces_the_items() {
        let mut source = ServiceMarketplace::default();
        source.follow_agent_kinds(&service_kinds());
        source.follow_agent_kinds(&[agent_kind("claude", "Claude", PackageState::Enabled)]);
        let names: Vec<String> = source
            .snapshot()
            .items
            .iter()
            .map(|item| item.name.clone())
            .collect();
        assert_eq!(names, ["Claude"]);
    }

    #[test]
    fn the_listed_accounts_stay_in_the_snapshot_when_the_kinds_are_read_again() {
        use massorca_protocol::methods::agents::AccountState as WireState;
        let mut source = ServiceMarketplace::default();
        source.follow_agent_kinds(&service_kinds());
        let summary = AccountSummary {
            id: "acct-1".to_owned(),
            label: "Work account".to_owned(),
            state: WireState::SignedIn,
            identity: None,
            active: true,
            projects: Vec::new(),
        };
        source.follow_accounts(&BTreeMap::from([("claude".to_owned(), Ok(vec![summary]))]));
        source.follow_agent_kinds(&service_kinds());
        let snapshot = source.snapshot();
        let rows = snapshot.accounts_for("claude");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            source.service_account(rows[0].id),
            Some(("claude".to_owned(), "acct-1".to_owned()))
        );
        assert_eq!(source.account_number("claude", "acct-1"), Some(rows[0].id));
    }

    #[test]
    fn the_service_source_refuses_the_changes_it_cannot_make_yet() {
        let mut source = ServiceMarketplace::default();
        source.follow_agent_kinds(&service_kinds());
        assert!(source.set_enabled(MarketplaceItemId(1), false).is_err());
        assert!(source.uninstall(MarketplaceItemId(1)).is_err());
        assert!(source.add_account("claude").is_err());
    }
}
