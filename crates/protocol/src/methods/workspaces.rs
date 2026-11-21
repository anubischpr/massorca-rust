//! Workspace methods: worktrees and folders created for a project.

use serde::{Deserialize, Serialize};

/// Params: [`WorkspaceCreateParams`]. Result: [`Workspace`].
pub const CREATE: &str = "workspace.create";
/// Params: [`WorkspaceListParams`]. Result: [`WorkspaceList`].
pub const LIST: &str = "workspace.list";
/// Params: [`WorkspaceRef`]. Result: [`Workspace`].
pub const SHOW: &str = "workspace.show";
/// Params: [`WorkspaceRemoveParams`]. Result: [`WorkspaceRemoved`].
pub const REMOVE: &str = "workspace.remove";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKind {
    Worktree,
    Folder,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceState {
    Ready,
    Removing,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Params of `workspace.create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCreateParams {
    pub project: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default)]
    pub existing_branch: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Params of `workspace.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// Result of `workspace.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceList {
    pub workspaces: Vec<Workspace>,
}

/// Selector for `workspace.show`: id, then exact name, then path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRef {
    pub workspace: String,
}

/// Params of `workspace.remove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRemoveParams {
    pub workspace: String,
    #[serde(default)]
    pub force: bool,
}

/// Result of `workspace.remove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRemoved {
    pub workspace: Workspace,
    pub branch_deleted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_kept_reason: Option<String>,
    /// True when the directory still exists and was left in place: a folder, or a directory
    /// git does not register as a worktree. Absent (false) when the directory was removed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub directory_kept: bool,
}

/// A workspace of a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub project_id: String,
    pub kind: WorkspaceKind,
    pub name: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_ref: Option<String>,
    pub created_branch: bool,
    pub state: WorkspaceState,
    pub missing: bool,
    pub created_at_ms: i64,
}
