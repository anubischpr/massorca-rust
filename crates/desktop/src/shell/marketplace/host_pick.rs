//! Which paired host a Marketplace item runs on, and the numbers of the requests sent to paired hosts. Each request carries
//! its number, and the store keeps only the newest answer per host.

use crate::model::marketplace::{HostTarget, MarketplaceItemId};
use crate::shell::Shell;

/// The requests the Marketplace has sent to paired hosts, numbered in the order they were sent.
#[derive(Debug, Default)]
pub(in crate::shell) struct HostRequests {
    issued: u64,
}

impl HostRequests {
    /// The number of a new request.
    pub(in crate::shell) fn request(&mut self) -> u64 {
        self.issued += 1;
        self.issued
    }
}

impl Shell {
    /// The paired host an item runs on; None for an item of This computer.
    pub(super) fn paired_host_of(&self, item: MarketplaceItemId) -> Option<String> {
        let found = self.model.marketplace().item(item)?;
        match HostTarget::from_key(&found.host.0)? {
            HostTarget::Paired(name) => Some(name),
            HostTarget::Local => None,
        }
    }
}
