//! Project methods: registered repositories and folders.

use serde::{Deserialize, Serialize};

/// Params: [`ProjectAddParams`]. Result: [`Project`].
pub const ADD: &str = "project.add";
/// Params: none. Result: [`ProjectList`].
pub const LIST: &str = "project.list";
/// Params: [`ProjectRef`]. Result: [`Project`].
pub const SHOW: &str = "project.show";
/// Params: [`ProjectRemoveParams`]. Result: [`Project`].
pub const REMOVE: &str = "project.remove";
/// Params: [`ProjectSetBaseParams`]. Result: [`Project`].
pub const SET_BASE: &str = "project.set_base";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    Git,
    Folder,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Params of `project.add`. Unset options take the service's defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAddParams {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ProjectKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspaces_root: Option<String>,
}

/// Selector for `project.show`: id, then exact name, then path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRef {
    pub project: String,
}

/// Params of `project.set_base`. A `None` base clears the project's base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSetBaseParams {
    /// Selected as `project.show` selects: id, then exact name, then path.
    pub project: String,
    pub base: Option<String>,
}

/// Params of `project.remove`. Selects by id, then exact name, then path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRemoveParams {
    pub project: String,
    /// Also forgets the project's workspace records. No file, folder, worktree or branch is touched.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub forget_workspaces: bool,
}

/// Result of `project.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectList {
    pub projects: Vec<Project>,
}

/// A registered project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub kind: ProjectKind,
    pub path: String,
    pub name: String,
    pub workspaces_root: String,
    pub added_at_ms: i64,
    /// The base a new workspace starts from when it names none; absent when the project has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_ref: Option<String>,
}
