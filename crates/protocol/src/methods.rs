//! Typed params and results of each method, grouped by feature area.
//!
//! Timestamps are Unix milliseconds in `i64` and ids are strings. Selector strings
//! (`project`, `workspace`) resolve by id, then exact name, then path.

pub mod agents;
pub mod automations;
pub mod calendar;
pub mod chat;
pub mod commands;
pub mod config;
pub mod devices;
pub mod events;
pub mod extension_points;
pub mod extensions;
pub mod git;
pub mod host;
pub mod hosted_services;
pub mod projects;
pub mod secrets;
pub mod self_update;
pub mod service;
pub mod skill_placement;
pub mod skills;
pub mod tasks;
pub mod team_room;
pub mod teams;
pub mod terminals;
pub mod usage;
pub mod workspaces;

/// Core never defines methods under this prefix; extensions use it for their own.
pub const RESERVED_EXTENSION_METHOD_PREFIX: &str = "ext.";
