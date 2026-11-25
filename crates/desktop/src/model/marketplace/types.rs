//! The plain data of the Marketplace: catalogue items, the hosts they run on and the agent
//! accounts they use. Only the sample source fills these with placeholder values.

use massorca_protocol::methods::extensions::ExtensionItem;

/// A catalogue item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarketplaceItemId(pub u32);

/// A host an item runs on, by its stable key (see `HostTarget`): `local` for This computer.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HostId(pub String);

/// An agent account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccountId(pub u32);

/// The kind of a catalogue item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Extension,
    Agent,
    Skill,
    Service,
    Theme,
}

impl ItemKind {
    pub const ALL: [ItemKind; 5] = [
        ItemKind::Extension,
        ItemKind::Agent,
        ItemKind::Skill,
        ItemKind::Service,
        ItemKind::Theme,
    ];

    pub fn singular(self) -> &'static str {
        match self {
            ItemKind::Extension => "Extension",
            ItemKind::Agent => "Agent",
            ItemKind::Skill => "Skill",
            ItemKind::Service => "Service",
            ItemKind::Theme => "Theme",
        }
    }

    pub fn plural(self) -> &'static str {
        match self {
            ItemKind::Extension => "Extensions",
            ItemKind::Agent => "Agents",
            ItemKind::Skill => "Skills",
            ItemKind::Service => "Services",
            ItemKind::Theme => "Themes",
        }
    }
}

/// Where an item stands on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemState {
    NotInstalled,
    Installed {
        enabled: bool,
        update: Option<String>,
    },
    Installing {
        percent: u8,
    },
    Failed {
        reason: String,
    },
    /// The item cannot run here; `requires` names what it needs, such as "MassOrca 1.6 or later".
    Incompatible {
        requires: String,
    },
}

impl ItemState {
    /// Installed, installing and failed items all count as installed.
    pub fn is_installed(&self) -> bool {
        matches!(
            self,
            ItemState::Installed { .. } | ItemState::Installing { .. } | ItemState::Failed { .. }
        )
    }
}

/// Whether a host answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostState {
    Connected,
    Unreachable,
    /// Any other state a host's last check left, by its word (such as "Checking" or "Sign-in needed").
    Other(&'static str),
}

impl HostState {
    pub fn word(self) -> &'static str {
        match self {
            HostState::Connected => "Connected",
            HostState::Unreachable => "Unreachable",
            HostState::Other(word) => word,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Host {
    pub id: HostId,
    pub name: String,
    pub state: HostState,
}

/// One catalogue entry and its state on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarketplaceItem {
    pub id: MarketplaceItemId,
    pub kind: ItemKind,
    pub name: String,
    pub publisher: String,
    /// Published by someone other than MassOrca.
    pub third_party: bool,
    pub version: String,
    pub description: String,
    /// What the item adds, such as "Rail item" or "2 commands".
    pub adds: Vec<String>,
    pub state: ItemState,
    pub host: HostId,
    pub developer: String,
    pub category: String,
    /// What the item asks for; None when it asks for nothing.
    pub permissions: Option<String>,
    /// A matching tool found on this computer. It is information, not an installed item.
    pub found_on_this_pc: Option<String>,
    /// Prompts that start with the item's tag, such as "@GitHub".
    pub examples: Vec<String>,
    /// The agent this item provides; only agent items have one.
    pub agent_kind: Option<String>,
    pub source_url: String,
    /// The version before this one, when the item was updated.
    pub previous_version: Option<String>,
    pub has_settings: bool,
    /// The extension package behind the item, as the service lists it (decision 0062); sample items have none.
    pub extension: Option<Box<ExtensionItem>>,
}

impl MarketplaceItem {
    /// Whether a lowercase query appears in the name, publisher, description or kind plural.
    fn matches_query(&self, query: &str) -> bool {
        [&self.name, &self.publisher, &self.description]
            .iter()
            .any(|text| text.to_lowercase().contains(query))
            || self.kind.plural().to_lowercase().contains(query)
    }
}

/// Whether an agent account can be used now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountState {
    SignedIn,
    Expired,
    NeedsSignIn,
    NotSetUp,
    Migrated,
    Conflict,
    /// The state cannot be read right now.
    Unknown,
}

impl AccountState {
    pub fn word(self) -> &'static str {
        match self {
            AccountState::SignedIn => "Signed in",
            AccountState::Expired => "Expired",
            AccountState::NeedsSignIn => "Needs sign-in",
            AccountState::NotSetUp => "Not set up",
            AccountState::Migrated => "Migrated",
            AccountState::Conflict => "Conflict",
            AccountState::Unknown => "Can't check",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentAccount {
    pub id: AccountId,
    pub agent_kind: String,
    pub label: String,
    pub state: AccountState,
    /// The identity the account is signed in as, when the agent reports one.
    pub identity: Option<String>,
    pub active: bool,
    pub used_by_projects: Vec<String>,
}

/// Where a sign-in to an agent account stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignInStep {
    Starting,
    WaitingBrowser { url: String },
    WaitingCode,
    Done,
    Failed { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignInFlow {
    pub agent_kind: String,
    pub step: SignInStep,
}

/// The config key the default agent will use. It does not exist yet (D-E).
pub const DEFAULT_AGENT_KEY: &str = "agents.default_kind";

/// Everything the Marketplace pages show, as one value from a source.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct MarketplaceSnapshot {
    pub items: Vec<MarketplaceItem>,
    pub hosts: Vec<Host>,
    pub accounts: Vec<AgentAccount>,
    pub sign_in: Option<SignInFlow>,
    /// False while `DEFAULT_AGENT_KEY` does not exist, so no agent can be made the default.
    pub default_agent_available: bool,
    /// Whether the service offers `extensions.rollback` for This computer's packages (`extensions-rollback/1`).
    pub rollback_offered: bool,
    /// Whether the service offers `extensions.log` for This computer's packages (`extensions-log/1`).
    pub log_offered: bool,
}

impl MarketplaceSnapshot {
    pub fn item(&self, id: MarketplaceItemId) -> Option<&MarketplaceItem> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn host(&self, id: &HostId) -> Option<&Host> {
        self.hosts.iter().find(|host| &host.id == id)
    }

    /// The accounts of one agent, in the order the source lists them.
    pub fn accounts_for(&self, agent_kind: &str) -> Vec<&AgentAccount> {
        self.accounts
            .iter()
            .filter(|account| account.agent_kind == agent_kind)
            .collect()
    }

    pub fn installed(&self) -> Vec<&MarketplaceItem> {
        self.items
            .iter()
            .filter(|item| item.state.is_installed())
            .collect()
    }

    /// The installed items that have an update waiting.
    pub fn updates(&self) -> Vec<&MarketplaceItem> {
        self.items
            .iter()
            .filter(|item| {
                matches!(
                    item.state,
                    ItemState::Installed {
                        update: Some(_),
                        ..
                    }
                )
            })
            .collect()
    }

    pub fn of_kind(&self, kind: ItemKind) -> Vec<&MarketplaceItem> {
        self.items.iter().filter(|item| item.kind == kind).collect()
    }

    /// Items whose name, publisher, description or kind plural contains the query, ignoring case
    /// and surrounding spaces. An empty query matches every item. Sorted by name, ignoring case.
    pub fn search(&self, query: &str) -> Vec<&MarketplaceItem> {
        let query = query.trim().to_lowercase();
        let mut found: Vec<&MarketplaceItem> = self
            .items
            .iter()
            .filter(|item| query.is_empty() || item.matches_query(&query))
            .collect();
        found.sort_by_cached_key(|item| (item.name.to_lowercase(), item.id));
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_and_host_labels_use_the_spec_words() {
        assert_eq!(
            ItemKind::ALL.map(ItemKind::plural),
            ["Extensions", "Agents", "Skills", "Services", "Themes"]
        );
        assert_eq!(ItemKind::Theme.singular(), "Theme");
        assert_eq!(
            [HostState::Connected.word(), HostState::Unreachable.word()],
            ["Connected", "Unreachable"]
        );
    }
}
