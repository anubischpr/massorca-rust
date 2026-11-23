//! macOS: `proc_pidinfo` for the resident set and for parents and names, `proc_listallpids` for the table.
//! Decision 0007 allows `unsafe` only in named platform modules such as this one.
#![allow(unsafe_code)]

use std::ffi::{c_int, c_void};

use crate::ProcessInfo;

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

pub(crate) fn resident_bytes(pid: u32) -> Option<u64> {
    // SAFETY: `proc_taskinfo` is plain integers, so all zeroes is a valid value.
    let mut info: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
    pid_info(pid, libc::PROC_PIDTASKINFO, &mut info).then_some(info.pti_resident_size)
}

pub(crate) fn process_table() -> Vec<ProcessInfo> {
    all_pids()
        .into_iter()
        .filter_map(|pid| {
            // SAFETY: `proc_bsdinfo` is plain integers and character arrays, so all zeroes is a valid value.
            let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
            if !pid_info(pid, libc::PROC_PIDTBSDINFO, &mut info) || info.pbi_status == SZOMB {
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
            })
        })
        .collect()
}

fn all_pids() -> Vec<u32> {
    // SAFETY: a null buffer of size 0 asks only for the number of pids; nothing is written.
    let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    let Ok(count) = usize::try_from(count) else {
        return Vec::new();
    };
    let mut pids: Vec<libc::pid_t> = vec![0; count + LIST_SLACK];
    let Ok(bytes) = c_int::try_from(pids.len() * size_of::<libc::pid_t>()) else {
        return Vec::new();
    };
    // SAFETY: `pids` is a writable buffer of `bytes` bytes; the call writes at most that many and returns how many
    // pids it wrote.
    let filled = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast::<c_void>(), bytes) };
    let Ok(filled) = usize::try_from(filled) else {
        return Vec::new();
    };
    pids.truncate(filled.min(pids.len()));
    pids.into_iter()
        .filter_map(|pid| u32::try_from(pid).ok())
        .filter(|pid| *pid != 0)
        .collect()
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
