//! Projects and workspaces over the store and git (ADR 0008).
//!
//! The service's handlers call [`Workspaces`] synchronously, inside `spawn_blocking`.
//! Methods return protocol types or a [`DomainError`] carrying a stable protocol code.
//! Nothing here publishes events: the handler publishes `workspace.removing` between
//! [`Workspaces::begin_remove`] and [`Workspaces::finish_remove`].

mod branch_prefix;
mod create;
mod error;
mod generated_names;
mod layout;
mod names;
mod paths;
mod project_icon_detection;
mod projects;
mod remove;
mod selectors;
pub mod setup;
pub mod setup_runner;
mod unregistered;
mod views;
mod workspaces;
mod worktree_state;

pub use branch_prefix::BranchPrefix;
pub use create::Created;
pub use error::DomainError;
pub use layout::WorkspaceLayout;
pub use projects::RemovedProject;
pub use remove::RemovalWaivers;
pub use workspaces::Workspaces;
