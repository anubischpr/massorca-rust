//! `git.stage`, `git.unstage` and `git.discard` (decisions 0010 and 0013).

use serde::{Deserialize, Serialize};

use super::is_false;

/// Params: [`GitPathsParams`]. Result: `null`.
pub const STAGE: &str = "git.stage";
/// Params: [`GitPathsParams`]. Result: `null`.
pub const UNSTAGE: &str = "git.unstage";
/// Params: [`GitDiscardParams`]. Result: [`GitDiscarded`].
pub const DISCARD: &str = "git.discard";

/// Workspace-relative paths, or every changed file with `all`; exactly one of the two.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitPathsParams {
    pub workspace: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub all: bool,
}

/// Each flag waives one refusal and nothing else (decision 0013).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiscardParams {
    pub workspace: String,
    /// Files as `git.status` lists them; directories are refused.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    /// Every file `git.status` lists, under the same waivers.
    #[serde(default, skip_serializing_if = "is_false")]
    pub all: bool,
    /// Delete untracked files, and files only the index knows (a staged addition with `include_staged`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub delete_untracked: bool,
    /// Restore from HEAD, dropping staged changes too; without it only unstaged edits are discarded.
    #[serde(default, skip_serializing_if = "is_false")]
    pub include_staged: bool,
    /// Return the plan without changing anything.
    #[serde(default, skip_serializing_if = "is_false")]
    pub dry_run: bool,
}

/// What a discard did, or with `dry_run` would do.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GitDiscarded {
    /// Restored from the index: their unstaged edits are gone, staged changes kept.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restored_from_index: Vec<String>,
    /// Restored from HEAD: staged and unstaged changes are gone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restored_from_head: Vec<String>,
    /// Removed from the index and from disk.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deleted: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub dry_run: bool,
}
