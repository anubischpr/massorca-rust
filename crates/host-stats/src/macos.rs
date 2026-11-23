//! macOS: `proc_pidinfo` for the resident set, parents, names and start times, `proc_listallpids` for the table.
//! Decision 0007 allows `unsafe` only in named platform modules such as this one.
#![allow(unsafe_code)]

use std::collections::BTreeSet;
use std::ffi::{c_int, c_void};

use crate::table::listed_until_it_fits;
use crate::{Listening, ProcessInfo, ProcessTable};

/// The socket table is not read on macOS. libc 0.2.191 declares neither `socket_fdinfo` nor the TCP socket
/// structures, and their layouts cannot be checked against a macOS SDK here, so a wrong offset could report wrong ports.
/// Until a macOS build verifies them, the table reads as unreadable and no port is listed (decision 0081).
pub(crate) fn listening_sockets(
    _table: &[ProcessInfo],
    _pids: &BTreeSet<u32>,
) -> Option<Listening> {
    None
}

/// `SZOMB` in `sys/proc.h`: the process ended and waits to be reaped.
const SZOMB: u32 = 5;
/// Room for processes started between the count and the listing.
const LIST_SLACK: usize = 64;

/// Fills `info` with `flavor` for `pid`; `true` only when the whole structure was written.
fn pid_info<T>(pid: u32, flavor: c_int, info: &mut T) -> bool {
    let (Ok(pid), Ok(size)) = (c_int::try_from(pid), c_int::try_from(size_of::<T>())) else {
        return false;
    };
    // SAFETY: `info` is a valid, writable `T` of `size` bytes, and `T` is the structure `flavor` writes
    // (`proc_taskinfo` for PROC_PIDTASKINFO, `proc_bsdinfo` for PROC_PIDTBSDINFO); the call writes at most `size`
    // bytes.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            flavor,
            0,
            std::ptr::from_mut(info).cast::<c_void>(),
            size,
        )
    };
    written == size
}

/// The resident set of `pid` in bytes, with no identity check. Only for a process whose identity is the caller's own.
pub(crate) fn resident_bytes(pid: u32) -> Option<u64> {
    // SAFETY: `proc_taskinfo` is plain integers, so all zeroes is a valid value.
    let mut info: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
    pid_info(pid, libc::PROC_PIDTASKINFO, &mut info).then_some(info.pti_resident_size)
}

/// The resident set of a process as `listed` recorded it. Its start time must match the listing before and after the
/// read, so a pid reused since the listing is never read (decision 0081).
pub(crate) fn resident_checked(listed: &ProcessInfo) -> Option<u64> {
    if !still_listed(listed) {
        return None;
    }
    // SAFETY: `proc_taskinfo` is plain integers, so all zeroes is a valid value.
    let mut info: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
    let bytes = pid_info(listed.pid, libc::PROC_PIDTASKINFO, &mut info)
        .then_some(info.pti_resident_size)?;
    still_listed(listed).then_some(bytes)
}

/// Whether the process `listed` names is live and has the start time its listing recorded.
fn still_listed(listed: &ProcessInfo) -> bool {
    bsd_info(listed.pid)
        .is_some_and(|info| info.pbi_status != SZOMB && Some(start_micros(&info)) == listed.start)
}

/// `PROC_PIDTBSDINFO` for `pid`: status, parent, names and start time.
fn bsd_info(pid: u32) -> Option<libc::proc_bsdinfo> {
    // SAFETY: `proc_bsdinfo` is plain integers and character arrays, so all zeroes is a valid value.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    pid_info(pid, libc::PROC_PIDTBSDINFO, &mut info).then_some(info)
}

/// Microseconds since the epoch at which the process started. Together with the pid it identifies the process.
fn start_micros(info: &libc::proc_bsdinfo) -> u64 {
    info.pbi_start_tvsec
        .saturating_mul(1_000_000)
        .saturating_add(info.pbi_start_tvusec)
}

pub(crate) fn process_table() -> ProcessTable {
    let (pids, complete) = all_pids();
    let processes = pids
        .into_iter()
        .filter_map(|pid| {
            let info = bsd_info(pid)?;
            if info.pbi_status == SZOMB {
                return None;
            }
            let name = c_name(&info.pbi_name);
            Some(ProcessInfo {
                pid,
                parent: (info.pbi_ppid != 0).then_some(info.pbi_ppid),
                name: if name.is_empty() {
                    c_name(&info.pbi_comm)
                } else {
                    name
                },
                session: None,
                start: Some(start_micros(&info)),
            })
        })
        .collect();
    ProcessTable {
        processes,
        complete,
    }
}

/// Every pid the kernel lists, and whether that is every pid it has. The listing is asked again with more room while
/// the count grows; one that still does not fit is reported partial, never cut short silently.
fn all_pids() -> (Vec<u32>, bool) {
    let Some(count) = pid_count() else {
        return (Vec::new(), false);
    };
    let (pids, complete) = listed_until_it_fits(count + LIST_SLACK, pids_filled);
    let pids = pids
        .into_iter()
        .filter_map(|pid| u32::try_from(pid).ok())
        .filter(|pid| *pid != 0)
        .collect();
    (pids, complete)
}

/// The number of pids the kernel lists now, `None` when it cannot say.
fn pid_count() -> Option<usize> {
    // SAFETY: a null buffer of size 0 asks only for the number of pids; nothing is written.
    let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    usize::try_from(count).ok()
}

/// The pids the kernel fills into `capacity` slots, `None` when the call fails.
fn pids_filled(capacity: usize) -> Option<Vec<libc::pid_t>> {
    let bytes = c_int::try_from(capacity.checked_mul(size_of::<libc::pid_t>())?).ok()?;
    let mut pids: Vec<libc::pid_t> = vec![0; capacity];
    // SAFETY: `pids` is a writable buffer of `bytes` bytes; the call writes at most that many and returns how many pids
    // it wrote.
    let filled = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast::<c_void>(), bytes) };
    let filled = usize::try_from(filled).ok()?.min(capacity);
    pids.truncate(filled);
    Some(pids)
}

/// The bytes before the first NUL, as text.
fn c_name(name: &[libc::c_char]) -> String {
    let bytes: Vec<u8> = name
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| byte.to_ne_bytes()[0])
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}
