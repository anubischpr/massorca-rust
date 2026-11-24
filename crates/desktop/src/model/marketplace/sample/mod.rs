//! The sample Marketplace: placeholder items, hosts and accounts that the pages show until a real
//! service is connected. Changes stay in this value.

use super::HostTarget;
use super::source::{MarketplaceSource, SourceError};
use super::types::{
    AccountId, AccountState, AgentAccount, Host, HostState, ItemState, MarketplaceItem,
    MarketplaceItemId, MarketplaceSnapshot, SignInFlow, SignInStep,
};

mod items;
#[cfg(test)]
mod tests;

use items::sample_items;

const SIGN_IN_URL: &str = "https://example.com/sign-in";
/// The code the sample rejects as expired; any other non-blank code completes the sign-in.
const EXPIRED_CODE: &str = "expired";

/// An in-memory source with placeholder data. Changes touch only this value.
#[derive(Clone, Debug)]
pub struct SampleMarketplaceSource {
    items: Vec<MarketplaceItem>,
    hosts: Vec<Host>,
    accounts: Vec<AgentAccount>,
    sign_in: Option<SignInFlow>,
    /// Accounts a live session uses; removing one is refused with `SourceError::Busy`.
    busy: Vec<AccountId>,
    /// The number the next signed-in account takes, for its id and its "Account N" label.
    next_account: u32,
}

impl Default for SampleMarketplaceSource {
    fn default() -> Self {
        Self {
            items: sample_items(),
            hosts: vec![
                Host {
                    id: HostTarget::Local.id(),
                    name: "This computer".to_owned(),
                    state: HostState::Connected,
                },
                Host {
                    id: HostTarget::Paired("remote-host".to_owned()).id(),
                    name: "Remote host".to_owned(),
                    state: HostState::Unreachable,
                },
            ],
            accounts: vec![
                AgentAccount {
                    id: AccountId(1),
                    agent_kind: "claude".to_owned(),
                    label: "Work account".to_owned(),
                    state: AccountState::SignedIn,
                    identity: Some("user@example.com".to_owned()),
                    active: true,
                    used_by_projects: vec!["Project one".to_owned()],
                },
                AgentAccount {
                    id: AccountId(2),
                    agent_kind: "claude".to_owned(),
                    label: "Second account".to_owned(),
                    state: AccountState::Expired,
                    identity: Some("second@example.com".to_owned()),
                    active: false,
                    used_by_projects: Vec::new(),
                },
            ],
            sign_in: None,
            busy: vec![AccountId(1)],
            next_account: 3,
        }
    }
}

impl SampleMarketplaceSource {
    /// The item, when it is installed. The sample changes only installed items.
    fn installed_mut(
        &mut self,
        item: MarketplaceItemId,
    ) -> Result<&mut MarketplaceItem, SourceError> {
        self.items
            .iter_mut()
            .find(|found| found.id == item && found.state.is_installed())
            .ok_or_else(not_installed)
    }

    fn account(&self, account: AccountId) -> Result<&AgentAccount, SourceError> {
        self.accounts
            .iter()
            .find(|found| found.id == account)
            .ok_or_else(no_such_account)
    }

    /// Adds the account a finished sign-in creates. The first account of its agent is active.
    fn add_signed_in_account(&mut self, agent_kind: &str) {
        let number = self.next_account;
        self.next_account += 1;
        let active = !self
            .accounts
            .iter()
            .any(|found| found.agent_kind == agent_kind);
        self.accounts.push(AgentAccount {
            id: AccountId(number),
            agent_kind: agent_kind.to_owned(),
            label: format!("Account {number}"),
            state: AccountState::SignedIn,
            identity: None,
            active,
            used_by_projects: Vec::new(),
        });
    }
}

impl MarketplaceSource for SampleMarketplaceSource {
    fn snapshot(&self) -> MarketplaceSnapshot {
        MarketplaceSnapshot {
            items: self.items.clone(),
            hosts: self.hosts.clone(),
            accounts: self.accounts.clone(),
            sign_in: self.sign_in.clone(),
            // The default-agent key does not exist yet, so the control stays disabled (D-E).
            default_agent_available: false,
        }
    }

    fn set_enabled(&mut self, item: MarketplaceItemId, enabled: bool) -> Result<(), SourceError> {
        match &mut self.installed_mut(item)?.state {
            ItemState::Installed {
                enabled: current, ..
            } => {
                *current = enabled;
                Ok(())
            }
            _ => Err(unavailable("This item cannot be turned on or off yet.")),
        }
    }

    fn uninstall(&mut self, item: MarketplaceItemId) -> Result<(), SourceError> {
        self.installed_mut(item)?.state = ItemState::NotInstalled;
        Ok(())
    }

    fn add_account(&mut self, agent_kind: &str) -> Result<(), SourceError> {
        if agent_kind != "claude" {
            return Err(unavailable("No such agent."));
        }
        self.sign_in = Some(SignInFlow {
            agent_kind: agent_kind.to_owned(),
            step: SignInStep::WaitingBrowser {
                url: SIGN_IN_URL.to_owned(),
            },
        });
        Ok(())
    }

    fn open_sign_in_page(&mut self) -> Result<(), SourceError> {
        match self.sign_in.as_mut() {
            Some(flow) if matches!(flow.step, SignInStep::WaitingBrowser { .. }) => {
                flow.step = SignInStep::WaitingCode;
                Ok(())
            }
            _ => Err(unavailable("No sign-in is waiting for its page to open.")),
        }
    }

    fn submit_sign_in_code(&mut self, code: &str) -> Result<(), SourceError> {
        let code = code.trim();
        let agent_kind = match self.sign_in.as_mut() {
            Some(flow) if matches!(flow.step, SignInStep::WaitingCode) => {
                if code.is_empty() {
                    return Err(unavailable("Enter the code from the sign-in page."));
                }
                if code == EXPIRED_CODE {
                    flow.step = SignInStep::Failed {
                        reason: "The code expired. Try again.".to_owned(),
                    };
                    return Ok(());
                }
                flow.step = SignInStep::Done;
                flow.agent_kind.clone()
            }
            _ => return Err(unavailable("No sign-in is waiting for a code.")),
        };
        self.add_signed_in_account(&agent_kind);
        Ok(())
    }

    fn retry_sign_in(&mut self) -> Result<(), SourceError> {
        match self.sign_in.as_mut() {
            Some(flow) if matches!(flow.step, SignInStep::Failed { .. }) => {
                flow.step = SignInStep::WaitingBrowser {
                    url: SIGN_IN_URL.to_owned(),
                };
                Ok(())
            }
            _ => Err(unavailable("Only a failed sign-in can be tried again.")),
        }
    }

    fn cancel_sign_in(&mut self) {
        self.sign_in = None;
    }

    fn activate_account(&mut self, account: AccountId) -> Result<(), SourceError> {
        let agent_kind = self.account(account)?.agent_kind.clone();
        for other in self
            .accounts
            .iter_mut()
            .filter(|other| other.agent_kind == agent_kind)
        {
            other.active = other.id == account;
        }
        Ok(())
    }

    fn remove_account(&mut self, account: AccountId) -> Result<(), SourceError> {
        if self.busy.contains(&account) {
            return Err(SourceError::Busy);
        }
        let index = self
            .accounts
            .iter()
            .position(|found| found.id == account)
            .ok_or_else(no_such_account)?;
        self.accounts.remove(index);
        Ok(())
    }

    fn check_account(&mut self, account: AccountId) -> Result<(), SourceError> {
        self.account(account).map(|_| ())
    }
}

fn unavailable(text: &str) -> SourceError {
    SourceError::Unavailable(text.to_owned())
}

fn not_installed() -> SourceError {
    unavailable("This item is not installed.")
}

fn no_such_account() -> SourceError {
    unavailable("No such account.")
}
