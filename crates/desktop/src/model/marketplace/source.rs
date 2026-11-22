//! The seam between the Marketplace pages and the service that owns installs and agent accounts.

use massorca_protocol::methods::agents::AgentKind;

use super::types::{AccountId, MarketplaceItemId, MarketplaceSnapshot};

/// Reads the Marketplace and changes it. The sample implements it in memory; the real service
/// implements it later. The account methods map to agent.accounts.list (the snapshot), add,
/// activate, remove and check, and to agent.accounts.progress later.
pub trait MarketplaceSource: std::fmt::Debug {
    /// Everything the pages show now.
    fn snapshot(&self) -> MarketplaceSnapshot;

    /// Takes the agent kinds the connected service lists. A source that lists no service's kinds ignores them.
    fn follow_agent_kinds(&mut self, _kinds: &[AgentKind]) {}

    /// Turns an installed item on or off.
    fn set_enabled(&mut self, item: MarketplaceItemId, enabled: bool) -> Result<(), SourceError>;

    /// Removes an installed item.
    fn uninstall(&mut self, item: MarketplaceItemId) -> Result<(), SourceError>;

    /// Starts the sign-in for an agent account.
    fn add_account(&mut self, agent_kind: &str) -> Result<(), SourceError>;

    fn open_sign_in_page(&mut self) -> Result<(), SourceError>;

    fn submit_sign_in_code(&mut self, code: &str) -> Result<(), SourceError>;

    fn retry_sign_in(&mut self) -> Result<(), SourceError>;

    /// Ends the sign-in, whatever step it is on.
    fn cancel_sign_in(&mut self);

    fn activate_account(&mut self, account: AccountId) -> Result<(), SourceError>;

    fn remove_account(&mut self, account: AccountId) -> Result<(), SourceError>;

    fn check_account(&mut self, account: AccountId) -> Result<(), SourceError>;
}

/// Why a source refused a change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceError {
    /// A live session uses the account, so it cannot be removed yet.
    Busy,
    /// The change cannot be made; the text is the reason to show.
    Unavailable(String),
}

impl SourceError {
    /// The words a page shows for this refusal.
    pub fn message(&self) -> String {
        match self {
            SourceError::Busy => {
                "A live session uses this account. Stop the session, then remove it.".to_owned()
            }
            SourceError::Unavailable(text) => text.clone(),
        }
    }
}

/// The source of a session with no service: nothing is listed and every change is refused.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnavailableMarketplace;

impl MarketplaceSource for UnavailableMarketplace {
    fn snapshot(&self) -> MarketplaceSnapshot {
        MarketplaceSnapshot::default()
    }

    fn set_enabled(&mut self, _: MarketplaceItemId, _: bool) -> Result<(), SourceError> {
        Err(offline())
    }

    fn uninstall(&mut self, _: MarketplaceItemId) -> Result<(), SourceError> {
        Err(offline())
    }

    fn add_account(&mut self, _: &str) -> Result<(), SourceError> {
        Err(offline())
    }

    fn open_sign_in_page(&mut self) -> Result<(), SourceError> {
        Err(offline())
    }

    fn submit_sign_in_code(&mut self, _: &str) -> Result<(), SourceError> {
        Err(offline())
    }

    fn retry_sign_in(&mut self) -> Result<(), SourceError> {
        Err(offline())
    }

    fn cancel_sign_in(&mut self) {}

    fn activate_account(&mut self, _: AccountId) -> Result<(), SourceError> {
        Err(offline())
    }

    fn remove_account(&mut self, _: AccountId) -> Result<(), SourceError> {
        Err(offline())
    }

    fn check_account(&mut self, _: AccountId) -> Result<(), SourceError> {
        Err(offline())
    }
}

/// The words a page shows when no service answers.
fn offline() -> SourceError {
    SourceError::Unavailable("The Marketplace needs a connected service.".to_owned())
}
