//! Platforms without a reader: nothing can be read, so every pid is unreadable and the table is empty and not complete.

use std::collections::BTreeSet;

use crate::{Listening, ProcessInfo, ProcessTable};

pub(crate) fn resident_bytes(_pid: u32) -> Option<u64> {
    None
}

pub(crate) fn listening_sockets(
    _table: &[ProcessInfo],
    _pids: &BTreeSet<u32>,
) -> Option<Listening> {
    None
}

/// A Unix platform without a reader cannot read a listed process either.
#[cfg(unix)]
pub(crate) fn resident_checked(_listed: &ProcessInfo) -> Option<u64> {
    None
}

pub(crate) fn process_table() -> ProcessTable {
    ProcessTable {
        processes: Vec::new(),
        complete: false,
    }
}
