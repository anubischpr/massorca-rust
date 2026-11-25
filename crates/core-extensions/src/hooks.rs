//! Remove guards and remove cleanups that other features register, such as the agents' live-session check and the
//! credential-home deletion. Each registration returns a handle that unregisters it when dropped.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use crate::error::HookError;

/// A guard, called with the package id.
pub type HookFn = dyn Fn(&str) -> Result<(), HookError> + Send + Sync;

/// A cleanup, called with the package that was removed. Its files are gone by then, so what it provided is passed in.
pub type CleanupFn = dyn Fn(&RemovedPackage) -> Result<(), HookError> + Send + Sync;

/// A package a removal has just deleted, as its cleanups see it (decision 0098).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedPackage {
    /// The package id.
    pub id: String,
    /// The agent kinds the package provided, read from its manifest before its files were deleted. Empty when the
    /// manifest could not be read (the removal reports that) and for a first-party id the catalogue does not vouch for,
    /// which never provided a kind.
    pub agent_kinds: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookKind {
    Guard,
    Cleanup,
}

/// What a hook runs: a guard takes the package id, a cleanup the removed package.
#[derive(Clone)]
pub(crate) enum HookRun {
    Guard(Arc<HookFn>),
    Cleanup(Arc<CleanupFn>),
}

impl HookRun {
    fn kind(&self) -> HookKind {
        match self {
            Self::Guard(_) => HookKind::Guard,
            Self::Cleanup(_) => HookKind::Cleanup,
        }
    }
}

struct Hook {
    id: u64,
    owner: String,
    run: HookRun,
}

#[derive(Default)]
pub(crate) struct Hooks {
    next: AtomicU64,
    list: Mutex<Vec<Hook>>,
}

impl fmt::Debug for Hooks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let list = self.list.lock().unwrap_or_else(PoisonError::into_inner);
        f.debug_list()
            .entries(list.iter().map(|hook| (hook.run.kind(), &hook.owner)))
            .finish()
    }
}

impl Hooks {
    pub(crate) fn register(self: &Arc<Self>, owner: String, run: HookRun) -> HookHandle {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.list
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Hook { id, owner, run });
        HookHandle {
            hooks: Arc::downgrade(self),
            id,
        }
    }

    /// The registered hooks of one kind, in registration order. Copied out, so a hook may register or drop handles.
    pub(crate) fn of(&self, kind: HookKind) -> Vec<(String, HookRun)> {
        self.list
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|hook| hook.run.kind() == kind)
            .map(|hook| (hook.owner.clone(), hook.run.clone()))
            .collect()
    }
}

/// Keeps a guard or cleanup registered; dropping it unregisters.
pub struct HookHandle {
    hooks: Weak<Hooks>,
    id: u64,
}

impl fmt::Debug for HookHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HookHandle").field("id", &self.id).finish()
    }
}

impl Drop for HookHandle {
    fn drop(&mut self) {
        if let Some(hooks) = self.hooks.upgrade() {
            hooks
                .list
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .retain(|hook| hook.id != self.id);
        }
    }
}
