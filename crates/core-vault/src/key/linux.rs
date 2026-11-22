//! The master key as an item in the Secret Service default collection. Unlocking is never requested, because a
//! prompt could hang a headless service; a missing, locked or slow service counts as unavailable.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use oo7::Secret;
use oo7::dbus::{Collection, Service};

use super::{LockReason, MasterKey};

const TIMEOUT: Duration = Duration::from_secs(2);
const DIR_ATTRIBUTE: &str = "massorca-vault-dir";
const APPLICATION: (&str, &str) = ("application", "massorca");

pub(super) enum Created {
    Stored,
    /// An item for this vault exists without a protection file; it is left alone.
    AlreadyThere,
    Unavailable,
}

/// Runs `work` on its own thread and runtime, so a caller inside a tokio runtime cannot deadlock or panic.
fn run<T: Send, F: Future<Output = Option<T>>>(work: impl FnOnce() -> F + Send) -> Option<T> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .ok()?;
                runtime
                    .block_on(async { tokio::time::timeout(TIMEOUT, work()).await })
                    .ok()
                    .flatten()
            })
            .join()
            .ok()
            .flatten()
    })
}

fn vault_id(vault_dir: &Path) -> Option<String> {
    let canonical = std::fs::canonicalize(vault_dir).ok()?;
    canonical.to_str().map(str::to_owned)
}

/// The default collection, only when it exists and is unlocked.
async fn unlocked_default() -> Option<Collection> {
    let service = Service::new().await.ok()?;
    let collection = service
        .with_alias(Service::DEFAULT_COLLECTION)
        .await
        .ok()??;
    (!collection.is_locked().await.ok()?).then_some(collection)
}

pub(super) fn create(vault_dir: &Path, key: &MasterKey) -> Created {
    let Some(id) = vault_id(vault_dir) else {
        return Created::Unavailable;
    };
    let attributes = [APPLICATION, (DIR_ATTRIBUTE, id.as_str())];
    let label = format!("MassOrca vault {id}");
    run(|| async {
        let collection = unlocked_default().await?;
        if !collection.search_items(&attributes).await.ok()?.is_empty() {
            return Some(Created::AlreadyThere);
        }
        collection
            .create_item(&label, &attributes, Secret::blob(key.bytes()), false, None)
            .await
            .ok()?;
        Some(Created::Stored)
    })
    .unwrap_or(Created::Unavailable)
}

pub(super) fn open(vault_dir: &Path) -> Result<MasterKey, LockReason> {
    let id = vault_id(vault_dir).ok_or(LockReason::ProtectionUnavailable)?;
    let attributes = [APPLICATION, (DIR_ATTRIBUTE, id.as_str())];
    run(|| async {
        let Some(collection) = unlocked_default().await else {
            return Some(Err(LockReason::ProtectionUnavailable));
        };
        let items = collection.search_items(&attributes).await.ok()?;
        let [item] = items.as_slice() else {
            return Some(Err(if items.is_empty() {
                LockReason::MissingKey
            } else {
                LockReason::Unwrap
            }));
        };
        let secret = item.secret().await.ok()?;
        Some(MasterKey::from_slice(secret.as_bytes()).ok_or(LockReason::Unwrap))
    })
    .unwrap_or(Err(LockReason::ProtectionUnavailable))
}

/// Removes this vault's item after a creation that could not record its protection.
pub(super) fn delete(vault_dir: &Path) {
    let Some(id) = vault_id(vault_dir) else {
        return;
    };
    let attributes = [APPLICATION, (DIR_ATTRIBUTE, id.as_str())];
    let _ = run(|| async {
        let collection = unlocked_default().await?;
        for item in collection.search_items(&attributes).await.ok()? {
            item.delete(None).await.ok()?;
        }
        Some(())
    });
}
