//! The git watch: what changes in a workspace's git state, followed live from the host (decision 0012, screen M-DF-02).

use std::fmt;
use std::sync::Arc;

use massorca_client::{ClientError, StreamItem};
use massorca_protocol::methods::git::watch::GitWatchEvent;

use crate::error::FfiError;
use crate::massorca::Host;
use crate::watch::{Feed, LinkState, Watched};
use crate::wire::word;

/// One step of a git watch: an item from the host, or a change of link.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum GitEvent {
    Item { item: GitItem },
    Link { link: LinkState },
}

impl From<LinkState> for GitEvent {
    fn from(link: LinkState) -> Self {
        Self::Link { link }
    }
}

/// What changed in a workspace's git state. After a change the screen re-reads `git_status`: this names the kind of
/// change and is not a diff.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct GitItem {
    /// True on the first item, which says the watch is set up.
    pub ready: bool,
    /// What changed, as words: `worktree`, `index`, `head` or `refs`. Empty on the first item.
    pub kinds: Vec<String>,
    /// Working-tree edits are not observed, only changes in git's own directory: poll `git_status` for those.
    pub degraded: bool,
    /// Why, when the host says.
    pub reason: Option<String>,
}

impl From<GitWatchEvent> for GitItem {
    fn from(event: GitWatchEvent) -> Self {
        Self {
            ready: event.ready,
            kinds: event.kinds.into_iter().map(word).collect(),
            degraded: event.degraded,
            reason: event.reason,
        }
    }
}

/// A live view of a workspace's git state. `close`, or dropping it, ends the connection.
#[derive(uniffi::Object)]
pub struct GitWatch {
    feed: Feed<GitEvent>,
}

impl fmt::Debug for GitWatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GitWatch").finish_non_exhaustive()
    }
}

#[uniffi::export]
impl GitWatch {
    /// The next event. `Ok(None)` once the watch is closed or the host ended it. An `Err` is final: the watch is over.
    pub async fn next(&self) -> Result<Option<GitEvent>, FfiError> {
        self.feed.next().await
    }

    /// Ends the watch and its connection. A `next` in progress returns `None`.
    pub fn close(&self) {
        self.feed.close();
    }
}

#[uniffi::export]
impl Host {
    /// Follows the git state of `workspace` (its id, name or path). The first item says whether working-tree edits are
    /// observed.
    pub fn watch_git(&self, workspace: String) -> Arc<GitWatch> {
        Arc::new(GitWatch {
            feed: self.follow(Watched::Git { workspace }, git_event),
        })
    }
}

/// Reads one `git.watch` item, field by field as the host sent it.
fn git_event(item: StreamItem) -> Result<Option<GitEvent>, ClientError> {
    let StreamItem::Event(value) = item else {
        return Ok(None);
    };
    let event: GitWatchEvent =
        serde_json::from_value(value).map_err(|error| ClientError::Protocol(error.to_string()))?;
    Ok(Some(GitEvent::Item {
        item: GitItem::from(event),
    }))
}
