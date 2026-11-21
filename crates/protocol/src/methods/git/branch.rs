//! `git.branches`, `git.branch_create`, `git.branch_switch`, `git.branch_rename` and `git.branch_delete`
//! (decisions 0010 and 0013). Branches are local branches, named without `refs/heads/`.

use serde::{Deserialize, Serialize};

/// Params: [`super::GitWorkspaceRef`]. Result: [`GitBranches`].
pub const BRANCHES: &str = "git.branches";
/// Params: [`GitBranchCreateParams`]. Result: [`GitBranch`].
pub const BRANCH_CREATE: &str = "git.branch_create";
/// Params: [`GitBranchSwitchParams`]. Result: [`GitBranchSwitched`].
pub const BRANCH_SWITCH: &str = "git.branch_switch";
/// Params: [`GitBranchRenameParams`]. Result: [`GitBranch`].
pub const BRANCH_RENAME: &str = "git.branch_rename";
/// Params: [`GitBranchDeleteParams`]. Result: [`GitBranchDeleted`].
pub const BRANCH_DELETE: &str = "git.branch_delete";

/// One local branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitBranch {
    pub name: String,
    /// The commit the branch points at.
    pub commit: String,
    /// The upstream, such as `origin/main`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    /// Commits on the branch that the upstream lacks; `None` without a reachable upstream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ahead: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behind: Option<u64>,
    /// The upstream is configured but its remote-tracking branch no longer exists.
    #[serde(default)]
    pub upstream_gone: bool,
    /// Checked out in the workspace the request named.
    #[serde(default)]
    pub current: bool,
    /// Checked out in another worktree of the repository.
    #[serde(default)]
    pub checked_out_elsewhere: bool,
}

/// The current branch first, then the others by name.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GitBranches {
    pub branches: Vec<GitBranch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitBranchCreateParams {
    pub workspace: String,
    pub name: String,
    /// Where the branch starts; defaults to HEAD. The branch gets no upstream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// Check the new branch out in the workspace.
    #[serde(default)]
    pub switch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitBranchSwitchParams {
    pub workspace: String,
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitBranchSwitched {
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitBranchRenameParams {
    pub workspace: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitBranchDeleteParams {
    pub workspace: String,
    pub branch: String,
    /// Delete a branch that is not merged.
    #[serde(default)]
    pub force: bool,
}

/// A deleted branch and the commit it pointed at, so it can be recreated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitBranchDeleted {
    pub branch: String,
    pub commit: String,
}
