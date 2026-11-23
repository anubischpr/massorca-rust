//! Platforms without a reader: nothing can be read, so every pid is unreadable and the table is empty.

use crate::ProcessInfo;

pub(crate) fn resident_bytes(_pid: u32) -> Option<u64> {
    None
}

pub(crate) fn process_table() -> Vec<ProcessInfo> {
    Vec::new()
}
