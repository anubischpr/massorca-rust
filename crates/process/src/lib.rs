//! The one way MassOrca starts a child process.
//!
//! On Windows a console program started by a process with no console of its own (the service runs detached) gets a new
//! console, and the default terminal application shows it as a window, one per start. Every [`Command`] made here
//! carries `CREATE_NO_WINDOW`, and [`creation_flags`] keeps it when a caller adds flags of its own, because
//! `CommandExt::creation_flags` replaces the flags instead of adding to them. A child that inherits a console keeps
//! using it; one that would get a new console gets one with no window.
//!
//! Nothing else in the workspace calls `Command::new` or `creation_flags` (see `tests/one_spawn_path.rs`).

use std::ffi::OsStr;
use std::process::Command;

/// The console program runs with a console but no window.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// The new process has no console at all.
pub const DETACHED_PROCESS: u32 = 0x0000_0008;
/// The new process is the root of a new process group.
pub const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
/// The new process starts with its main thread suspended.
pub const CREATE_SUSPENDED: u32 = 0x0000_0004;

/// A command for `program` that never opens a console window on Windows.
#[must_use]
pub fn command(program: impl AsRef<OsStr>) -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut command = Command::new(program);
    #[cfg(windows)]
    creation_flags(&mut command, 0);
    command
}

/// Sets `flags` on `command` and keeps `CREATE_NO_WINDOW` with them. Flags only exist on Windows; elsewhere this does
/// nothing.
#[cfg_attr(not(windows), allow(unused_variables))]
pub fn creation_flags(command: &mut Command, flags: u32) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(flags | CREATE_NO_WINDOW);
    }
}
