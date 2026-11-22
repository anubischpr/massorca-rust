//! `git.watch` (decision 0012): one watcher per work tree, shared by its subscribers and stopped with the last one.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, Weak};

use massorca_core_watch::{WatchHandle, WatchKind, WatchSpec};
use massorca_protocol::methods::git::GitWorkspaceRef;
use massorca_protocol::methods::git::watch::{self, GitWatchEvent, WatchKind as WireKind};
use massorca_protocol::{ErrorBody, capabilities, codes};
use tokio::sync::{broadcast, mpsc};

use super::{blocking, git_error, repository};
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::method::{StreamSource, encode};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

/// Bursts buffered per subscriber; one that falls further behind gets a catch-all burst instead.
const BURSTS: usize = 64;

/// The watcher of one work tree and the bursts it publishes.
struct Shared {
    _handle: WatchHandle,
    bursts: broadcast::Sender<BTreeSet<WatchKind>>,
    degraded: Option<String>,
}

static WATCHERS: LazyLock<Mutex<HashMap<PathBuf, Weak<Shared>>>> = LazyLock::new(Mutex::default);

pub(super) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![registries.methods.register_stream(
        Owner::Core,
        watch::WATCH,
        capabilities::GIT,
        Permission::Read,
        subscribe,
    )?])
}

async fn subscribe(ctx: RequestCtx, params: GitWorkspaceRef) -> Result<StreamSource, ErrorBody> {
    let repository = repository(&ctx, params.workspace).await?;
    let spec = blocking(move || {
        let (git_dir, common_dir) = repository
            .git
            .repository_dirs(&repository.dir)
            .map_err(git_error)?;
        Ok(WatchSpec {
            worktree: canonical(repository.dir)?,
            git_dir: canonical(git_dir)?,
            common_dir: canonical(common_dir)?,
        })
    })
    .await?;
    let shared = blocking(move || shared(spec)).await?;
    // Subscribed before `ready` is sent, so no burst after it is missed.
    let mut bursts = shared.bursts.subscribe();
    let (items, source) = mpsc::channel(BURSTS);
    let ready = GitWatchEvent {
        ready: true,
        kinds: Vec::new(),
        degraded: shared.degraded.is_some(),
        reason: shared.degraded.clone(),
    };
    tokio::spawn(async move {
        // Holding the watcher here ties its life to this stream.
        let _shared = shared;
        if send(&items, &ready).await.is_err() {
            return;
        }
        loop {
            let kinds = tokio::select! {
                () = items.closed() => break,
                burst = bursts.recv() => match burst {
                    Ok(kinds) => kinds.into_iter().map(wire).collect(),
                    // A subscriber that lagged has missed bursts; telling it everything changed is always safe.
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        vec![WireKind::Worktree, WireKind::Index, WireKind::Head, WireKind::Refs]
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                },
            };
            let event = GitWatchEvent {
                ready: false,
                kinds,
                degraded: false,
                reason: None,
            };
            if send(&items, &event).await.is_err() {
                break;
            }
        }
    });
    Ok(source)
}

/// The running watcher for `spec`, or a new one when none is running.
fn shared(spec: WatchSpec) -> Result<Arc<Shared>, ErrorBody> {
    let mut watchers = WATCHERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    watchers.retain(|_, shared| shared.strong_count() > 0);
    if let Some(running) = watchers.get(&spec.worktree).and_then(Weak::upgrade) {
        return Ok(running);
    }
    let (bursts, _) = broadcast::channel(BURSTS);
    let publish = bursts.clone();
    let key = spec.worktree.clone();
    let handle = massorca_core_watch::watch(spec, move |kinds| {
        let _ = publish.send(kinds);
    })
    .map_err(|error| ErrorBody::new(codes::GIT_WATCH_UNAVAILABLE, error.to_string()))?;
    let shared = Arc::new(Shared {
        degraded: handle.degraded().map(str::to_owned),
        _handle: handle,
        bursts,
    });
    watchers.insert(key, Arc::downgrade(&shared));
    Ok(shared)
}

async fn send(items: &mpsc::Sender<serde_json::Value>, event: &GitWatchEvent) -> Result<(), ()> {
    let value = encode(event).map_err(|_| ())?;
    items.send(value).await.map_err(|_| ())
}

fn wire(kind: WatchKind) -> WireKind {
    match kind {
        WatchKind::Worktree => WireKind::Worktree,
        WatchKind::Index => WireKind::Index,
        WatchKind::Head => WireKind::Head,
        WatchKind::Refs => WireKind::Refs,
    }
}

fn canonical(path: PathBuf) -> Result<PathBuf, ErrorBody> {
    dunce::canonicalize(&path).map_err(|error| {
        ErrorBody::new(
            codes::PATH_NOT_FOUND,
            format!("cannot resolve {}: {error}", path.display()),
        )
    })
}
