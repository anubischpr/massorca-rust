//! Where a project's default workspaces go (`workspaces.layout`).

/// `InProject` puts a git project's worktrees in `<project>/.worktrees` (decision 0183); `Nested` gives each project a
/// folder of its own under the default root; `Flat` puts every project's workspaces directly in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkspaceLayout {
    /// `<project>/.worktrees` for a git project; a folder project, which has no worktrees, takes `Nested`'s root.
    #[default]
    InProject,
    /// `<default root>/<project name>`.
    Nested,
    /// `<default root>`, so a workspace is `<default root>/<workspace name>`.
    Flat,
}
