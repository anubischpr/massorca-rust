//! Reads the executing host's processes with native calls, never a spawned program (decision 0081).
//!
//! - [`memory_of`] and [`sample_memory`]: a process's resident memory, as [`METRIC`] names it.
//! - [`processes`]: the live process table (pid, parent, name, and the session on Linux); on Unix, `descendants` walks
//!   the tree below one pid.
//! - Windows: `job_members` and `sample_job` list and read a terminal's job.
//!
//! Each platform's calls live in one module; only the Windows and macOS ones use `unsafe` (decision 0007).
//! A process that exited or cannot be opened is never an error: it reads as `None`, or is left out of the table.

mod memory;
mod sockets;
mod table;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod other;
#[cfg(any(target_os = "linux", test))]
mod proc_net;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as sys;
#[cfg(target_os = "macos")]
use macos as sys;
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
use other as sys;
#[cfg(windows)]
use windows as sys;

#[cfg(windows)]
pub use memory::sample_job;
#[cfg(unix)]
pub use memory::sample_listed;
pub use memory::{METRIC, MemorySample, Metric, ProcessMemory, memory_of, sample_memory};
pub use sockets::{Listening, ListeningSocket, listening_tcp};
#[cfg(unix)]
pub use table::descendants;
pub use table::{ProcessInfo, ProcessTable, processes};
#[cfg(windows)]
pub use table::{job_members, job_still_holds};
