//! Which developer tools the service's host has (PLAT-08, decision 0116): each program is found on the host's `PATH`
//! and asked for its version, bounded by a timeout. Nothing else runs: no shell is started, no network is reached.

pub mod catalog;
pub mod probe;
pub mod tool_cache;
