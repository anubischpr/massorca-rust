//! Git methods on a workspace (capability `git/1`, decisions 0010-0015). One submodule per command group.
//!
//! Paths are workspace-relative, UTF-8, with `/` separators.

pub mod diff;
pub mod index;
pub mod log;
pub mod status;

use serde::{Deserialize, Serialize};

/// Params of a git method that only names a workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitWorkspaceRef {
    pub workspace: String,
}

/// Added and removed line counts of one side of a change. Binary files count no lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineCounts {
    pub added: u64,
    pub removed: u64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub binary: bool,
}

/// How a path changed between two sides (HEAD, index, working tree).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}
