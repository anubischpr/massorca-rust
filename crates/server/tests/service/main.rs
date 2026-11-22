//! Service integration tests: the service runs in-process and is driven through the client crate; raw frames cover
//! the cases the client cannot produce.

mod capabilities;
mod common;
mod concurrency;
mod config;
mod config_keys;
mod core_methods;
mod events;
mod frames;
mod git_branches;
mod git_commit;
mod git_diff;
mod git_fixture;
mod git_index;
mod git_log;
mod git_remote;
mod git_status;
mod git_watch;
mod handshake;
mod lifecycle;
mod network;
mod projects;
mod registries;
mod tasks;
mod teams;
mod workspaces;
