//! Typed params and results of each method, grouped by feature area.
//!
//! Timestamps are Unix milliseconds in `i64` and ids are strings. Selector strings
//! (`project`, `workspace`) resolve by id, then exact name, then path.

pub mod agents;
pub mod config;
pub mod devices;
pub mod events;
pub mod git;
pub mod projects;
pub mod secrets;
pub mod service;
pub mod terminals;
pub mod workspaces;

/// Core never defines methods under this prefix; extensions use it for their own.
pub const RESERVED_EXTENSION_METHOD_PREFIX: &str = "ext.";
