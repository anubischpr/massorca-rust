//! Resident memory of a set of processes.

#[cfg(any(unix, test))]
use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[cfg(any(unix, test))]
use crate::ProcessInfo;
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

/// Reads each process of `pids` as `table` listed it (Unix). A pid whose identity no longer matches its listing is
/// counted in `unreadable` and never summed: a pid reused since the listing is another process (decision 0081). A pid
/// the listing does not name is unreadable too. Never fails as a whole.
#[cfg(unix)]
#[must_use]
pub fn sample_listed(table: &[ProcessInfo], pids: &[u32]) -> MemorySample {
    sample_with(table, pids, sys::resident_checked)
}

/// The sample of `pids`, each read by `read` from its entry in `table`. A pid read twice is read once.
#[cfg(any(unix, test))]
fn sample_with(
    table: &[ProcessInfo],
    pids: &[u32],
    read: impl Fn(&ProcessInfo) -> Option<u64>,
) -> MemorySample {
    let listed: BTreeMap<u32, &ProcessInfo> = table.iter().map(|info| (info.pid, info)).collect();
    let mut processes = Vec::new();
    let mut unreadable = 0u32;
    for pid in pids.iter().copied().collect::<BTreeSet<u32>>() {
        match listed.get(&pid).and_then(|info| read(info)) {
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

/// Reads every process of a terminal's job (Windows). Each process is checked to be in `job` on the handle that reads
/// it, so a pid reused after the listing is not read. `None` when the job cannot be listed; then the job's memory is
/// unknown, not zero.
#[cfg(windows)]
#[must_use]
pub fn sample_job(job: &win32job::Job) -> Option<MemorySample> {
    sys::sample_job(job)
}

#[cfg(test)]
mod tests {
    use super::{ProcessMemory, sample_with};
    use crate::ProcessInfo;

    fn listed(pid: u32, start: Option<u64>) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent: None,
            name: format!("p{pid}"),
            session: None,
            start,
        }
    }

    #[test]
    fn a_pid_whose_listing_no_longer_matches_is_unreadable_and_never_summed() {
        // Pid 11 was reused: the reader refuses it because its start time differs from its listing.
        let table = [listed(10, Some(100)), listed(11, Some(200))];
        let read = |info: &ProcessInfo| (info.start == Some(100)).then_some(5);
        let sample = sample_with(&table, &[10, 11, 12, 10], read);
        assert_eq!(sample.processes, [ProcessMemory { pid: 10, bytes: 5 }]);
        assert_eq!(
            sample.unreadable, 2,
            "11 was reused and 12 was never listed"
        );
        assert_eq!(sample.total_bytes(), 5);
    }
}
