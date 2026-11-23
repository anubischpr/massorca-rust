//! The live process table: pids, parents and names.

use std::collections::{BTreeMap, BTreeSet};

use crate::sys;

/// One live process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    /// The parent's pid when the platform records one. On Windows it may name a parent that has exited.
    pub parent: Option<u32>,
    /// The program's name, without its directory.
    pub name: String,
}

/// Every live process this user can see, in one pass. Ended processes (Unix zombies) are left out. Empty when the
/// table cannot be read.
#[must_use]
pub fn processes() -> Vec<ProcessInfo> {
    sys::process_table()
}

/// `root` and every process below it in `table`, by parent id, in pid order. `root` is included only when it is in
/// the table. A cycle (a reused pid named as a parent) is walked once.
#[must_use]
pub fn descendants(root: u32, table: &[ProcessInfo]) -> Vec<u32> {
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

#[cfg(test)]
mod tests {
    use super::{ProcessInfo, descendants};

    fn entry(pid: u32, parent: Option<u32>) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent,
            name: format!("p{pid}"),
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
