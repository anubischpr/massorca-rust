//! Workspace methods: worktrees and folders created for a project.

use serde::{Deserialize, Serialize};

/// Params: [`WorkspaceCreateParams`]. Result: [`Workspace`].
pub const CREATE: &str = "workspace.create";
/// Params: [`WorkspaceListParams`]. Result: [`WorkspaceList`].
pub const LIST: &str = "workspace.list";
/// Params: [`WorkspaceRef`]. Result: [`Workspace`].
pub const SHOW: &str = "workspace.show";
/// Params: [`WorkspaceCurrentParams`]. Result: [`Workspace`].
pub const CURRENT: &str = "workspace.current";
/// Params: [`WorkspaceRemoveParams`]. Result: [`WorkspaceRemoved`].
pub const REMOVE: &str = "workspace.remove";
/// Params: [`WorkspaceUpdateParams`]. Result: [`Workspace`].
pub const UPDATE: &str = "workspace.update";
/// Params: [`WorkspaceRef`]. Result: [`WorkspaceCheck`].
pub const CHECK: &str = "workspace.check";
/// Params: [`WorkspaceRenameParams`]. Result: [`Workspace`].
pub const RENAME: &str = "workspace.rename";

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

/// Params of `workspace.create`. A blank or omitted `name` asks the service for a generated one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCreateParams {
    pub project: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default)]
    pub existing_branch: bool,
    /// Fetches the base's branch first; a failed fetch creates from the local copy.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub refresh_base: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Does not run the project's setup script in the new worktree (decision 0086).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_setup: bool,
    /// A taken typed name, branch or folder moves on to `name-2` .. `name-100` instead of being refused; the result
    /// carries the name used. Only for a service-derived branch and folder (`workspace-unique-name/1`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unique_name: bool,
    /// The client derived `name` itself (from a prompt, say), so `workspace.rename` may replace it later. Ignored with
    /// `branch` or `existing_branch`, whose branch is the person's (`workspace-rename/1`, decision 0111).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub name_generated: bool,
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
    /// Linked worktrees of a project that no workspace records, under its workspaces root or elsewhere.
    /// They are reported, never removed. Absent when there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unregistered: Vec<UnregisteredWorktree>,
}

/// A linked worktree of a project with no workspace row: usually left by a create that stopped
/// after `worktree add`, or made outside Massorca.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnregisteredWorktree {
    pub project_id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Git's lock reason; `Some("")` when locked without one. Absent when unlocked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<String>,
    /// Git's prunable reason. Absent when git does not prune it. Git before 2.31 never prints one,
    /// so a folder that is gone is reported with git's wording for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prunable: Option<String>,
}

/// Selector for `workspace.show`: id, then exact name, then path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRef {
    pub workspace: String,
}

/// Params of `workspace.current`: the workspace whose path holds `path`, which must be absolute on the service's host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCurrentParams {
    pub path: String,
}

/// Params of `workspace.remove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRemoveParams {
    pub workspace: String,
    /// Discards uncommitted and untracked changes in a registered worktree.
    #[serde(default)]
    pub force: bool,
    /// Deletes ignored files in a registered worktree, which git would otherwise delete unseen.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub discard_ignored: bool,
    /// Forgets a row whose directory git does not recognise as its worktree; the directory stays.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub forget_orphan: bool,
    /// Removes the workspaces of delegated children first, deepest first, each with the same checks and waivers
    /// (decision 0110). Without it a workspace with child workspaces is refused with `workspace.has_children`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub with_children: bool,
    /// Removes the worktree even when its archive script fails or times out (decision 0095).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_failed_archive: bool,
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
    /// The delegated children's workspaces removed first under `with_children`, deepest first. Absent when none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_children: Vec<WorkspaceRemoved>,
    /// What became of the project's archive script; absent when there is none or the workspace is a folder (0095).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive: Option<WorkspaceArchive>,
}

/// What a removal did with the project's archive script, which runs in the worktree before anything is stopped or
/// removed. A failure refuses the removal unless `allow_failed_archive` waives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum WorkspaceArchive {
    /// The script ran in the service terminal `terminal` and exited 0.
    Succeeded { terminal: String },
    /// The script's content is not trusted, so it did not run; the removal went ahead.
    Untrusted { reason: String, hash: String },
    /// The script failed, and `allow_failed_archive` let the removal go ahead.
    Failed {
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        terminal: Option<String>,
    },
    /// An outcome from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Params of `workspace.update`. A field left out keeps its value; an empty `comment` clears it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceUpdateParams {
    /// Id, name or path.
    pub workspace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// Params of `workspace.rename` (decision 0111): replaces a name the service chose, and its unpublished branch. The
/// folder keeps its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRenameParams {
    /// Id, name or path.
    pub workspace: String,
    /// The new name, checked as a created name is; a taken one moves on to `name-2` .. `name-100`.
    pub name: String,
    /// Must be true: only a workspace whose `name_generated` holds is renamed.
    #[serde(default)]
    pub from_generated: bool,
    /// Refuses the rename with `workspace.branch_changed` unless the worktree is still on this branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_branch: Option<String>,
}

/// Result of `workspace.check`. `ok` holds only when every check passes; `reason` names the first that did not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCheck {
    pub id: String,
    pub name: String,
    pub kind: WorkspaceKind,
    pub path: String,
    /// Whether the folder is there.
    pub exists: bool,
    /// Whether the folder can be listed.
    pub readable: bool,
    /// Whether git lists the worktree. Absent for a folder, and when git could not answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registered: Option<bool>,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
    /// Git's lock reason for the worktree; `Some("")` when locked without one. Absent when
    /// unlocked; create and remove results do not read it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<String>,
    /// Git's prunable reason for the worktree. Absent when git does not prune it, and always on
    /// Git before 2.31. `missing` is still answered by the directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prunable: Option<String>,
    /// Only in the reply of a create that asked for `refresh_base`: what the refresh did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_refresh: Option<BaseRefresh>,
    /// The owner's comment, set by `workspace.update`; absent when none is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Only in the reply of a create whose new worktree has a setup script: what became of it (decision 0086).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<WorkspaceSetup>,
    /// The service chose the name (decision 0111), so `workspace.rename` may replace it; a typed name is never renamed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub name_generated: bool,
}

/// What a create did with the project's setup script. A setup that cannot start never fails the create.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum WorkspaceSetup {
    /// The script runs in the service terminal `terminal`, which every client can list and attach to.
    Started { terminal: String },
    /// The create asked for `no_setup`.
    Skipped,
    /// The script's content is not trusted, so it did not run (decision 0095). `hash` is what `project.trust_script`
    /// takes once the owner has read the script.
    Untrusted { reason: String, hash: String },
    /// The script could not be started; the workspace exists without it.
    Failed { reason: String },
    /// An outcome from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// What a create's `refresh_base` did. A failed fetch never fails the create: it starts from the local copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum BaseRefresh {
    /// The base's remote-tracking branch was fetched; `moved` says whether it changed.
    Fetched { moved: bool },
    /// The base is a local branch, a tag or a commit: there was nothing to fetch.
    NotRemote,
    /// The fetch failed; the workspace starts from the local copy of the base.
    Failed { reason: String },
    /// An outcome from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}
