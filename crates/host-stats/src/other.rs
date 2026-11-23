//! Platforms without a reader: nothing can be read, so every pid is unreadable and the table is empty and not complete.

use crate::{ProcessInfo, ProcessTable};

pub(crate) fn resident_bytes(_pid: u32) -> Option<u64> {
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
