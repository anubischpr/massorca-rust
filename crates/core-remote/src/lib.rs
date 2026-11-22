//! Remote hosts of MassOrca: Tailscale status, SSH bootstrap and the systemd user unit.
//!
//! Decisions 0043 to 0045. Everything is synchronous: the CLI calls it from `spawn_blocking`.

pub mod bootstrap;
pub mod ssh;
pub mod tailscale;

mod process;

pub use process::hide_window;
