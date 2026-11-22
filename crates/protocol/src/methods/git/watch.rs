//! `git.watch`: a stream that says what changed in a workspace, so a client asks again instead of polling (decision 0012).

use serde::{Deserialize, Serialize};

use super::is_false;

/// Params: [`super::GitWorkspaceRef`]. Stream items: [`GitWatchEvent`].
pub const WATCH: &str = "git.watch";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchKind {
    /// A file under the working tree: refresh status and diffs.
    Worktree,
    /// The index: refresh status and staged diffs.
    Index,
    /// HEAD or an operation in progress: refresh status, the branch and history.
    Head,
    /// Branches and remote-tracking refs: refresh branch lists, ahead and behind.
    Refs,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// The first item has `ready: true` and no kinds; it says whether working-tree edits are observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitWatchEvent {
    #[serde(default, skip_serializing_if = "is_false")]
    pub ready: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<WatchKind>,
    /// Working-tree edits are not observed (only git-directory changes are); poll status at your own pace.
    #[serde(default, skip_serializing_if = "is_false")]
    pub degraded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
