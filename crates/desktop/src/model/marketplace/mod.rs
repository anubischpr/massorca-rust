//! The Marketplace as plain data: catalogue items, hosts and agent accounts, behind a source trait
//! that the sample implements. No GPUI types here, so the rules test as Rust.

mod sample;
mod source;
mod types;

pub use sample::SampleMarketplaceSource;
pub use source::{MarketplaceSource, SourceError, UnavailableMarketplace};
pub use types::{
    AccountId, AccountState, AgentAccount, DEFAULT_AGENT_KEY, Host, HostId, HostState, ItemKind,
    ItemState, MarketplaceItem, MarketplaceItemId, MarketplaceSnapshot, SignInFlow, SignInStep,
};
