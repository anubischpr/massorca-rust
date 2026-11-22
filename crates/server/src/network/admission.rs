//! Limits on network connections that have not authenticated yet (decision 0040): fewer of them than of
//! authenticated ones, a few per address, and one small frame each until `welcome`.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use massorca_net::PreAuthOpener;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Connections between accept and `welcome`, from every address.
pub(crate) const MAX_PENDING: usize = 32;
/// Connections between accept and `welcome` from one address.
pub(crate) const MAX_PENDING_PER_ADDRESS: usize = 4;
/// The largest frame read before authentication; a `hello` is far smaller.
pub(crate) const PRE_AUTH_MAX_FRAME: u32 = 64 * 1024;

#[derive(Debug)]
pub(crate) struct Admission {
    slots: Arc<Semaphore>,
    per_address: Mutex<HashMap<IpAddr, usize>>,
}

impl Default for Admission {
    fn default() -> Self {
        Self {
            slots: Arc::new(Semaphore::new(MAX_PENDING)),
            per_address: Mutex::default(),
        }
    }
}

/// One unauthenticated connection's place. Dropping it frees the place; [`Pending::authenticated`] also opens the
/// connection's reader.
#[derive(Debug)]
pub(crate) struct Pending {
    admission: Arc<Admission>,
    address: IpAddr,
    _slot: OwnedSemaphorePermit,
    opener: Option<PreAuthOpener>,
}

impl Admission {
    /// A place for a new connection from `address`, or `None` when either limit is reached.
    pub(crate) fn try_enter(self: &Arc<Self>, address: IpAddr) -> Option<Pending> {
        let slot = Arc::clone(&self.slots).try_acquire_owned().ok()?;
        let mut per_address = self.lock();
        let count = per_address.entry(address).or_default();
        if *count >= MAX_PENDING_PER_ADDRESS {
            return None;
        }
        *count += 1;
        Some(Pending {
            admission: Arc::clone(self),
            address,
            _slot: slot,
            opener: None,
        })
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<IpAddr, usize>> {
        self.per_address
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl Pending {
    pub(crate) fn set_opener(&mut self, opener: PreAuthOpener) {
        self.opener = Some(opener);
    }

    /// The connection authenticated: its reader may take full frames, and its place is free.
    pub(crate) fn authenticated(self) {
        if let Some(opener) = &self.opener {
            opener.open();
        }
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        let mut per_address = self.admission.lock();
        if let Some(count) = per_address.get_mut(&self.address) {
            *count -= 1;
            if *count == 0 {
                per_address.remove(&self.address);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    fn ip(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(100, 64, 0, last))
    }

    #[test]
    fn one_address_holds_few_unauthenticated_connections() {
        let admission = Arc::new(Admission::default());
        let held: Vec<_> = (0..MAX_PENDING_PER_ADDRESS)
            .map(|_| admission.try_enter(ip(1)).expect("admitted"))
            .collect();
        assert!(admission.try_enter(ip(1)).is_none());
        assert!(
            admission.try_enter(ip(2)).is_some(),
            "other addresses still get in"
        );
        drop(held);
        assert!(
            admission.try_enter(ip(1)).is_some(),
            "places come back when connections end"
        );
    }

    #[test]
    fn unauthenticated_connections_are_capped_overall() {
        let admission = Arc::new(Admission::default());
        let held: Vec<_> = (0..MAX_PENDING)
            .map(|n| {
                admission
                    .try_enter(ip(u8::try_from(n).unwrap()))
                    .expect("admitted")
            })
            .collect();
        assert!(admission.try_enter(ip(200)).is_none());
        drop(held);
        assert!(admission.try_enter(ip(200)).is_some());
    }

    #[test]
    fn authenticating_frees_the_place() {
        let admission = Arc::new(Admission::default());
        let held: Vec<_> = (0..MAX_PENDING_PER_ADDRESS)
            .map(|_| admission.try_enter(ip(1)).expect("admitted"))
            .collect();
        for pending in held {
            pending.authenticated();
        }
        assert!(admission.try_enter(ip(1)).is_some());
    }
}
