//! The Marketplace as plain data: catalogue items, hosts and agent accounts, behind a source trait
//! that the sample implements. No GPUI types here, so the rules test as Rust.

mod accounts;
mod agent_links;
mod catalogue;
mod item_page;
mod sample;
mod source;
mod types;

pub use accounts::{AccountView, account_views};
pub use agent_links::{AgentLink, installed_agent_links};
pub use catalogue::{count_line, counts, heading, intro, listed, no_match_text};
pub use item_page::{
    Part, advanced_rows, information, parts, settings_rows, state_chip, uninstall_body,
};
pub use sample::SampleMarketplaceSource;
pub use source::{MarketplaceSource, SourceError, UnavailableMarketplace};
pub use types::{
    AccountId, AccountState, AgentAccount, DEFAULT_AGENT_KEY, Host, HostId, HostState, ItemKind,
    ItemState, MarketplaceItem, MarketplaceItemId, MarketplaceSnapshot, SignInFlow, SignInStep,
};
