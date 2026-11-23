//! Resident memory of a set of processes.

use std::collections::BTreeSet;

use crate::sys;

/// What a memory reading measures on this platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Metric {
    /// Windows: the pages of the process now in physical memory, shared ones included.
    WorkingSet,
    /// Linux and macOS: the resident set size, shared pages included.
    Resident,
}

/// The metric this platform's readings use.
pub const METRIC: Metric = if cfg!(windows) {
    Metric::WorkingSet
} else {
    Metric::Resident
};

/// One process's reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessMemory {
    pub pid: u32,
    pub bytes: u64,
}

/// Readings of a set of processes taken one after the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySample {
    pub metric: Metric,
    /// The processes that could be read, by pid.
    pub processes: Vec<ProcessMemory>,
    /// Processes that could not be read: exited, or not open to this user.
    pub unreadable: u32,
}

impl MemorySample {
    /// The plain sum of the readings; a page shared by two processes counts twice, as each process reports it.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.processes
            .iter()
            .fold(0u64, |total, process| total.saturating_add(process.bytes))
    }
}

/// The resident memory of `pid` in bytes, as [`METRIC`] measures it. `None` when the process exited or cannot be read.
#[must_use]
pub fn memory_of(pid: u32) -> Option<u64> {
    sys::resident_bytes(pid)
}

/// Reads every process of `pids` once; a pid listed twice is read once. Never fails as a whole: a process that cannot
/// be read is counted in `unreadable`.
#[must_use]
pub fn sample_memory(pids: &[u32]) -> MemorySample {
    let mut processes = Vec::new();
    let mut unreadable = 0u32;
    for pid in pids.iter().copied().collect::<BTreeSet<u32>>() {
        match memory_of(pid) {
            Some(bytes) => processes.push(ProcessMemory { pid, bytes }),
            None => unreadable = unreadable.saturating_add(1),
        }
    }
    MemorySample {
        metric: METRIC,
        processes,
        unreadable,
    }
}
