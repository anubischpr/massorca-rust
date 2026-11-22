//! `git.status`: changed files grouped as staged, unstaged, untracked and conflicted, with the branch state.

use serde::{Deserialize, Serialize};

use super::{ChangeKind, LineCounts, is_false};

/// Params: [`GitStatusParams`]. Result: [`GitStatus`].
pub const STATUS: &str = "git.status";

/// Entries listed when the caller sets no limit (decision 0011).
pub const DEFAULT_STATUS_LIMIT: u32 = 1_000;
/// The largest limit a caller may ask for.
pub const MAX_STATUS_LIMIT: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStatusParams {
    pub workspace: String,
    /// At most this many entries; `None` means [`DEFAULT_STATUS_LIMIT`]. Clamped to [`MAX_STATUS_LIMIT`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStatus {
    pub branch: BranchState,
    /// A merge, rebase or other multi-step operation that stopped midway.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<GitOperation>,
    /// Sorted by path; at most the requested limit.
    pub entries: Vec<StatusEntry>,
    /// Counts over every entry, including those past the limit.
    pub totals: StatusTotals,
    /// More entries exist than `entries` lists.
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BranchState {
    /// The checked-out branch; `None` when HEAD is detached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// The commit HEAD points at; `None` before the first commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oid: Option<String>,
    /// The upstream, such as `origin/main`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    /// Commits on HEAD that the upstream lacks; `None` without a reachable upstream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ahead: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behind: Option<u64>,
    /// The upstream is configured but its remote-tracking branch no longer exists.
    #[serde(default, skip_serializing_if = "is_false")]
    pub upstream_gone: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitOperation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    /// `git am` stopped midway.
    ApplyMailbox,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEntry {
    pub path: String,
    /// The path before a rename or copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    /// HEAD against the index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staged: Option<ChangeKind>,
    /// The index against the working tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unstaged: Option<ChangeKind>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub untracked: bool,
    /// Set for an unmerged path; `staged` and `unstaged` are then unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<ConflictKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staged_lines: Option<LineCounts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unstaged_lines: Option<LineCounts>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub submodule: bool,
    /// The path is not valid UTF-8 and is shown with replacement characters; methods that take a path refuse it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub path_lossy: bool,
}

/// Which side changed what, in git's unmerged codes (`UU`, `AA`, `DD`, `AU`, `UA`, `DU`, `UD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    BothModified,
    BothAdded,
    BothDeleted,
    AddedByUs,
    AddedByThem,
    DeletedByUs,
    DeletedByThem,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StatusTotals {
    pub staged: u64,
    pub unstaged: u64,
    pub untracked: u64,
    pub conflicted: u64,
}
