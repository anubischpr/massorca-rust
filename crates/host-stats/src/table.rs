//! The live process table: pids, parents, names and start times.

use crate::sys;

/// One live process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    /// The parent's pid when the platform records one. On Windows it may name a parent that has exited.
    pub parent: Option<u32>,
    /// The program's name, without its directory.
    pub name: String,
    /// Linux: the session id, which groups a terminal's processes. `None` elsewhere.
    pub session: Option<u32>,
    /// What identifies the process while its pid is held: Linux clock ticks since boot (`starttime`), macOS
    /// microseconds since the epoch. A pid reused since the listing has another start time. `None` on Windows, where a
    /// job's handle identifies its members.
    pub start: Option<u64>,
}

/// The process table and whether it lists every live process. `complete` is `false` when the platform could not list
/// them all, or could not list any: a process missing from the table may belong to a terminal, so the answers built
/// from it are partial (decision 0081). A short table is never passed on as a whole one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessTable {
    pub processes: Vec<ProcessInfo>,
    pub complete: bool,
}

/// Every live process this user can see, in one pass. Ended processes (Unix zombies) are left out.
#[must_use]
pub fn processes() -> ProcessTable {
    sys::process_table()
}

/// The pids in a terminal's job (Windows), however many there are. `None` when the job cannot be listed.
#[cfg(windows)]
#[must_use]
pub fn job_members(job: &win32job::Job) -> Option<Vec<u32>> {
    sys::job_members(job)
}

/// The pids of `pids` that are still members of `job` (Windows), each checked on its own handle.
#[cfg(windows)]
#[must_use]
pub fn job_still_holds(job: &win32job::Job, pids: &[u32]) -> Vec<u32> {
    sys::job_still_holds(job, pids)
}

/// `root` and every process below it in `table`, by parent id, in pid order. Unix only: Windows parent ids may name a
/// parent that exited, whose pid another process now holds. `root` is included only when it is in
/// the table. A cycle (a reused pid named as a parent) is walked once.
#[cfg(any(unix, test))]
#[must_use]
pub fn descendants(root: u32, table: &[ProcessInfo]) -> Vec<u32> {
    use std::collections::{BTreeMap, BTreeSet};

    let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for process in table {
        if let Some(parent) = process.parent.filter(|parent| *parent != process.pid) {
            children.entry(parent).or_default().push(process.pid);
        }
    }
    let mut found = BTreeSet::new();
    if table.iter().any(|process| process.pid == root) {
        found.insert(root);
    }
    let mut pending = vec![root];
    while let Some(pid) = pending.pop() {
        for &child in children.get(&pid).into_iter().flatten() {
            if found.insert(child) {
                pending.push(child);
            }
        }
    }
    found.into_iter().collect()
}

/// How many times a listing is asked again with more room, when an answer fills every slot: the number of entries grew
/// while it was taken.
#[cfg(any(target_os = "macos", test))]
const LIST_ATTEMPTS: usize = 8;

/// Asks `query` for the entries that fit in `capacity` slots, with twice the room each time an answer fills every slot,
/// at most `LIST_ATTEMPTS` times. Returns the entries and whether they are all of them. `false` when every attempt
/// filled its slots (the last answer is kept) or a query failed (then nothing is listed): the caller reports the
/// listing as partial instead of cutting it short silently.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn listed_until_it_fits<T>(
    first_capacity: usize,
    mut query: impl FnMut(usize) -> Option<Vec<T>>,
) -> (Vec<T>, bool) {
    let mut capacity = first_capacity.max(1);
    let mut last = Vec::new();
    for _ in 0..LIST_ATTEMPTS {
        let Some(entries) = query(capacity) else {
            return (Vec::new(), false);
        };
        if entries.len() < capacity {
            return (entries, true);
        }
        last = entries;
        capacity = capacity.saturating_mul(2);
    }
    (last, false)
}

#[cfg(test)]
mod tests {
    use super::{ProcessInfo, descendants};

    fn entry(pid: u32, parent: Option<u32>) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent,
            name: format!("p{pid}"),
            session: None,
            start: None,
        }
    }

    #[test]
    fn the_walk_follows_parent_ids_and_stops_at_a_cycle() {
        let table = [
            entry(1, None),
            entry(10, Some(1)),
            entry(11, Some(10)),
            entry(12, Some(11)),
            entry(20, Some(1)),
            entry(30, Some(31)),
            entry(31, Some(30)),
        ];
        assert_eq!(descendants(10, &table), [10, 11, 12]);
        assert_eq!(descendants(1, &table), [1, 10, 11, 12, 20]);
        assert_eq!(descendants(30, &table), [30, 31]);
        assert_eq!(descendants(99, &table), Vec::<u32>::new());
    }
}

#[cfg(test)]
mod listing_tests {
    use super::listed_until_it_fits;

    #[test]
    fn a_listing_that_grows_while_it_is_taken_is_read_whole() {
        let all: Vec<u32> = (1..=100).collect();
        let (pids, complete) =
            listed_until_it_fits(10, |capacity| Some(all[..capacity.min(all.len())].to_vec()));
        assert!(complete);
        assert_eq!(pids, all);
    }

    #[test]
    fn a_listing_that_never_fits_is_reported_partial_not_cut_short_silently() {
        let (pids, complete) =
            listed_until_it_fits(4, |capacity| Some((1..=capacity as u32).collect()));
        assert!(!complete, "{} entries and still no room", pids.len());
        assert!(pids.len() > 4, "the last answer is kept, flagged partial");
    }

    #[test]
    fn a_failed_query_lists_nothing_and_is_not_complete() {
        let (pids, complete) = listed_until_it_fits::<u32>(4, |_| None);
        assert!(pids.is_empty());
        assert!(!complete);
    }
}
