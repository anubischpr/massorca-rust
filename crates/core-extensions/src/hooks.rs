//! Remove guards and remove cleanups that other features register, such as the agents' live-session check and the
//! credential-home deletion. Each registration returns a handle that unregisters it when dropped.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use crate::error::HookError;

/// A guard or cleanup, called with the package id.
pub type HookFn = dyn Fn(&str) -> Result<(), HookError> + Send + Sync;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookKind {
    Guard,
    Cleanup,
}

struct Hook {
    id: u64,
    kind: HookKind,
    owner: String,
    run: Arc<HookFn>,
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
            .entries(list.iter().map(|hook| (hook.kind, &hook.owner)))
            .finish()
    }
}

impl Hooks {
    pub(crate) fn register(
        self: &Arc<Self>,
        kind: HookKind,
        owner: String,
        run: Arc<HookFn>,
    ) -> HookHandle {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.list
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Hook {
                id,
                kind,
                owner,
                run,
            });
        HookHandle {
            hooks: Arc::downgrade(self),
            id,
        }
    }

    /// The registered hooks of one kind, in registration order. Copied out, so a hook may register or drop handles.
    pub(crate) fn of(&self, kind: HookKind) -> Vec<(String, Arc<HookFn>)> {
        self.list
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|hook| hook.kind == kind)
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
