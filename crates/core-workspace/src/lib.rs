//! Projects and workspaces over the store and git (ADR 0008).
//!
//! The service's handlers call [`Workspaces`] synchronously, inside `spawn_blocking`.
//! Methods return protocol types or a [`DomainError`] carrying a stable protocol code.
//! Nothing here publishes events: the handler publishes `workspace.removing` between
//! [`Workspaces::begin_remove`] and [`Workspaces::finish_remove`].

mod error;
mod names;
mod paths;
mod projects;
mod selectors;
mod views;
mod workspaces;

pub use error::DomainError;
pub use workspaces::Workspaces;
