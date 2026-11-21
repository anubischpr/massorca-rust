//! Capabilities: named, versioned feature groups that the service offers in `welcome` (ADR 0002).

use crate::owner::Owner;
use crate::registry::{RegistrationError, RegistrationHandle, Registry};

/// Keeps a capability offered. Dropping it withdraws the capability from every later handshake.
pub type CapabilityHandle = RegistrationHandle;

/// The capabilities the service offers. Clones share one table.
#[derive(Clone, Debug, Default)]
pub struct CapabilityRegistry {
    entries: Registry<()>,
}

impl CapabilityRegistry {
    /// Offers `name` on behalf of `owner` for as long as the handle lives.
    pub fn register(
        &self,
        name: &str,
        owner: Owner,
    ) -> Result<CapabilityHandle, RegistrationError> {
        self.entries.register(owner, name, ())
    }

    /// The names offered now, sorted.
    pub fn names(&self) -> Vec<String> {
        self.entries
            .list()
            .iter()
            .map(|entry| entry.key.clone())
            .collect()
    }
}
