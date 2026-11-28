//! `git.rebase_base`: rebases a worktree's branch onto its base branch (decision 0181), with a typed outcome.

use serde::{Deserialize, Serialize};

use super::is_false;

/// Params: [`GitRebaseBaseParams`]. Result: [`GitRebased`].
pub const REBASE_BASE: &str = "git.rebase_base";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitRebaseBaseParams {
    pub workspace: String,
    /// Fetch the base's remote-tracking branch first; a failed fetch refuses the rebase.
    #[serde(default, skip_serializing_if = "is_false")]
    pub refresh: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum GitRebased {
    /// The branch now sits on its base; `head` is its new commit.
    Rebased { head: String },
    /// The base is already in the branch's history; nothing changed.
    UpToDate,
    /// The rebase stopped on conflicts and was aborted: HEAD and the worktree are as they were. `paths` are the
    /// conflicted files.
    Conflict { paths: Vec<String> },
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}
