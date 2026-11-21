//! `git.fetch`, `git.pull`, `git.push` and `git.abort`: typed outcomes, never a prompt (decision 0013).

use serde::{Deserialize, Serialize};

use super::is_false;
use super::status::GitOperation;

/// Params: [`GitFetchParams`]. Result: [`GitFetched`].
pub const FETCH: &str = "git.fetch";
/// Params: [`GitPullParams`]. Result: [`GitPulled`].
pub const PULL: &str = "git.pull";
/// Params: [`GitPushParams`]. Result: [`GitPushed`].
pub const PUSH: &str = "git.push";
/// Params: [`super::GitWorkspaceRef`]. Result: [`GitAborted`].
pub const ABORT: &str = "git.abort";

/// How long a remote operation may run when the caller sets no limit; it is also the largest limit.
pub const MAX_REMOTE_TIMEOUT_MS: u64 = 5 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitFetchParams {
    pub workspace: String,
    /// The remote to fetch; `None` means the current branch's remote, else `origin`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    /// Fetch every remote.
    #[serde(default, skip_serializing_if = "is_false")]
    pub all: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GitFetched {
    /// Remote-tracking refs that moved, appeared or were pruned.
    pub updated: Vec<RefChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefChange {
    /// Such as `origin/main`.
    pub name: String,
    /// `None` for a new ref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
    /// `None` for a pruned ref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullMode {
    /// Move the branch only if no merge is needed; otherwise report `diverged` and change nothing.
    #[default]
    FfOnly,
    Merge,
    Rebase,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitPullParams {
    pub workspace: String,
    #[serde(default)]
    pub mode: PullMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum GitPulled {
    UpToDate,
    FastForwarded {
        from: String,
        to: String,
    },
    Merged {
        commit: String,
    },
    Rebased {
        head: String,
    },
    /// `ff_only` found both sides with new commits; nothing changed.
    Diverged {
        ahead: u64,
        behind: u64,
    },
    /// The merge or rebase stopped; resolve the files, or undo it with `git.abort`.
    Conflicts {
        paths: Vec<String>,
    },
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitPushParams {
    pub workspace: String,
    /// The remote for a branch without an upstream; defaults to `origin`. Ignored when an upstream exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    /// Overwrite the remote branch only if it still points at this commit (`--force-with-lease`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_with_lease: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitPushed {
    /// The remote branch, such as `origin/feature`.
    pub remote_branch: String,
    pub outcome: PushOutcome,
    /// The push created the upstream of the branch.
    #[serde(default, skip_serializing_if = "is_false")]
    pub upstream_set: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PushOutcome {
    Created,
    FastForward,
    Forced,
    UpToDate,
    /// The remote refused; `reason` is git's short reason, such as `non-fast-forward` or `stale info`.
    Rejected {
        reason: String,
    },
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitAborted {
    pub operation: GitOperation,
}
