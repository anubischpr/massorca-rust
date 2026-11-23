//! Reads the executing host's processes with native calls, never a spawned program (decision 0081).
//!
//! - [`memory_of`] and [`sample_memory`]: a process's resident memory, as [`METRIC`] names it.
//! - [`processes`] and [`descendants`]: the live process table (pid, parent, name) and the tree below one pid.
//!
//! Each platform's calls live in one module; only the Windows and macOS ones use `unsafe` (decision 0007).
//! A process that exited or cannot be opened is never an error: it reads as `None`, or is left out of the table.

mod memory;
mod table;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod other;
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

pub use memory::{METRIC, MemorySample, Metric, ProcessMemory, memory_of, sample_memory};
pub use table::{ProcessInfo, descendants, processes};
