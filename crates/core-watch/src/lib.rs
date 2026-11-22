//! Change notifications for one git work tree (decision 0012): the operating system's file events, sorted by
//! what they touch and debounced, with nothing polled while the tree is quiet.

mod classify;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

pub use classify::{WatchKind, WatchSpec};

/// A burst is reported once it has been quiet this long...
pub const QUIET: Duration = Duration::from_millis(200);
/// ...or once it has lasted this long, so a long build still reports a steady trickle.
pub const MAX_DELAY: Duration = Duration::from_secs(1);

/// Why a work tree cannot be watched at all.
#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("cannot watch {path:?}: {source}")]
    Unwatchable {
        path: PathBuf,
        #[source]
        source: notify::Error,
    },
}

/// A live watch; dropping it stops the watchers and the debounce thread.
#[derive(Debug)]
pub struct WatchHandle {
    _watchers: Vec<RecommendedWatcher>,
    degraded: Option<String>,
}

impl WatchHandle {
    /// Why working-tree edits are not observed, when they are not; git-directory changes still are.
    pub fn degraded(&self) -> Option<&str> {
        self.degraded.as_deref()
    }
}

/// Watches `spec` and calls `on_change` with each debounced burst's kinds, on a thread of its own.
/// Fails only when the git directory cannot be watched; a work tree that cannot is reported by `degraded`.
pub fn watch<F>(spec: WatchSpec, on_change: F) -> Result<WatchHandle, WatchError>
where
    F: Fn(BTreeSet<WatchKind>) + Send + 'static,
{
    let (events, received) = mpsc::channel::<PathBuf>();
    let watcher = |events: mpsc::Sender<PathBuf>| {
        notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
            // inotify also reports opens and reads; a reader such as `git status` changes nothing.
            if let Ok(event) = result.as_ref()
                && matches!(event.kind, notify::EventKind::Access(_))
            {
                return;
            }
            if let Ok(event) = result {
                for path in event.paths {
                    let _ = events.send(path);
                }
            }
        })
    };
    let mut watchers = Vec::new();
    let mut add = |path: &Path, mode: RecursiveMode| -> Result<(), WatchError> {
        let unwatchable = |source| WatchError::Unwatchable {
            path: path.to_path_buf(),
            source,
        };
        let mut one = watcher(events.clone()).map_err(unwatchable)?;
        one.watch(path, mode).map_err(unwatchable)?;
        watchers.push(one);
        Ok(())
    };
    add(&spec.git_dir, RecursiveMode::Recursive)?;
    if spec.common_dir != spec.git_dir {
        add(&spec.common_dir, RecursiveMode::Recursive)?;
    }
    let degraded = match add(&spec.worktree, RecursiveMode::Recursive) {
        Ok(()) => None,
        // Linux runs out of inotify watches on huge trees; git-directory events still arrive.
        Err(error) => Some(error.to_string()),
    };
    drop(events);
    thread::Builder::new()
        .name("massorca-git-watch".to_owned())
        .spawn(move || debounce(&spec, &received, &on_change))
        .map_err(|source| WatchError::Unwatchable {
            path: PathBuf::new(),
            source: notify::Error::io(source),
        })?;
    Ok(WatchHandle {
        _watchers: watchers,
        degraded,
    })
}

/// Collects kinds until the burst is quiet for [`QUIET`] or [`MAX_DELAY`] has passed, then reports them.
/// Ends when every watcher is dropped, which closes the channel.
fn debounce<F>(spec: &WatchSpec, received: &mpsc::Receiver<PathBuf>, on_change: &F)
where
    F: Fn(BTreeSet<WatchKind>),
{
    let mut pending = BTreeSet::new();
    let mut started: Option<Instant> = None;
    loop {
        // Idle (`None`): block until the next event, with no timer running.
        let wait = started.map(|start| QUIET.min(MAX_DELAY.saturating_sub(start.elapsed())));
        let next = match wait {
            None => received.recv().map_err(|_| RecvTimeoutError::Disconnected),
            Some(wait) => received.recv_timeout(wait),
        };
        match next {
            Ok(path) => {
                if let Some(kind) = spec.classify(&path) {
                    pending.insert(kind);
                    started.get_or_insert_with(Instant::now);
                }
                if started.is_some_and(|start| start.elapsed() >= MAX_DELAY) {
                    flush(&mut pending, &mut started, on_change);
                }
            }
            Err(RecvTimeoutError::Timeout) => flush(&mut pending, &mut started, on_change),
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn flush<F>(pending: &mut BTreeSet<WatchKind>, started: &mut Option<Instant>, on_change: &F)
where
    F: Fn(BTreeSet<WatchKind>),
{
    *started = None;
    if !pending.is_empty() {
        on_change(std::mem::take(pending));
    }
}
