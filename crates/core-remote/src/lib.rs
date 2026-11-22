//! Remote hosts of MassOrca: Tailscale status, SSH bootstrap and the systemd user unit.
//!
//! Decisions 0043 to 0045. Everything is synchronous: the CLI calls it from `spawn_blocking`.

pub mod bootstrap;
pub mod ssh;
pub mod tailscale;

mod process;

/// The suspended spawn into a kill-on-close job, shared by core-git and the service (decision 0062).
#[cfg(windows)]
pub mod windows_job;
