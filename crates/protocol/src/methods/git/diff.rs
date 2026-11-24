//! `git.diff` lists the changed files of a comparison; `git.diff_file` returns one file's patch (decision 0011).

use serde::{Deserialize, Serialize};

use super::{ChangeKind, LineCounts, is_false};

/// Params: [`GitDiffParams`]. Result: [`GitDiff`].
pub const DIFF: &str = "git.diff";
/// Params: [`GitDiffFileParams`]. Result: [`GitDiffFile`].
pub const DIFF_FILE: &str = "git.diff_file";

/// Files listed when the caller sets no limit.
pub const DEFAULT_DIFF_LIMIT: u32 = 1_000;
pub const MAX_DIFF_LIMIT: u32 = 10_000;
/// Patch bytes returned when the caller sets no cap.
pub const DEFAULT_PATCH_BYTES: u32 = 1 << 20;
pub const MAX_PATCH_BYTES: u32 = 4 << 20;

/// What a diff compares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum DiffTarget {
    /// The index against the working tree: unstaged changes.
    Worktree,
    /// HEAD against the index: staged changes.
    Staged,
    /// HEAD against the working tree: everything not committed.
    Head,
    /// One revision against another, such as a branch against its base.
    Range {
        from: String,
        /// Defaults to `HEAD`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiffParams {
    pub workspace: String,
    #[serde(flatten)]
    pub target: DiffTarget,
    /// Clamped to [`MAX_DIFF_LIMIT`]; `None` means [`DEFAULT_DIFF_LIMIT`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiff {
    /// Sorted by path; at most the requested limit.
    pub files: Vec<DiffEntry>,
    /// Counts over every changed file, including those past the limit.
    pub totals: DiffTotals,
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffEntry {
    pub path: String,
    /// The path before a rename or copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    pub change: ChangeKind,
    pub lines: LineCounts,
    #[serde(default, skip_serializing_if = "is_false")]
    pub path_lossy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DiffTotals {
    pub files: u64,
    pub added: u64,
    pub removed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiffFileParams {
    pub workspace: String,
    #[serde(flatten)]
    pub target: DiffTarget,
    pub path: String,
    /// For a rename, the old path from `git.diff`, so the patch shows the rename rather than an addition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    /// Clamped to [`MAX_PATCH_BYTES`]; `None` means [`DEFAULT_PATCH_BYTES`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiffFile {
    pub path: String,
    /// The unified patch, git's own format; absent for a binary file and when nothing changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub binary: bool,
    /// The patch was cut at the last line break before the byte cap.
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
    /// Sizes of the two sides in bytes, for a binary file; `None` where that side does not exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_size: Option<u64>,
    /// Set for a submodule (gitlink) change in place of a text patch: the commits the parent records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submodule: Option<SubmodulePointer>,
}

/// A submodule's change, which git records as the commit a parent points at, not as file contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmodulePointer {
    /// The commit the old side records; absent where that side has no submodule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_commit: Option<String>,
    /// The commit the new side records: the submodule's checked-out HEAD for a working-tree side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_commit: Option<String>,
    /// Whether the submodule's working tree has changes; absent when the new side is not a checked-out working tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty: Option<bool>,
    /// Git's one-line form, `Subproject commit <old>..<new>`, with one side when only one is known.
    pub summary: String,
}
