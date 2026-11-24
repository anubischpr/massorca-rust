//! Where a project's default workspaces go under the default root (`workspaces.layout`).

/// `Nested` gives each project a folder of its own under the default root; `Flat` puts every
/// project's workspaces directly in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkspaceLayout {
    /// `<default root>/<project name>`.
    #[default]
    Nested,
    /// `<default root>`, so a workspace is `<default root>/<workspace name>`.
    Flat,
}
