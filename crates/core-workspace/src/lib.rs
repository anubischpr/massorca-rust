//! Projects and workspaces over the store and git (ADR 0008).
//!
//! Methods return protocol types or a [`DomainError`] carrying a stable protocol code.

mod error;
mod names;
mod paths;

pub use error::DomainError;
