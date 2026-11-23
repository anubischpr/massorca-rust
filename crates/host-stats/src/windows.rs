//! Windows: the working set through `K32GetProcessMemoryInfo`, the table through a Toolhelp32 snapshot.
//! Decision 0007 allows `unsafe` only in named platform modules such as this one.
#![allow(unsafe_code)]

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, STILL_ACTIVE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::ProcessInfo;

/// A handle this module opened, closed when dropped.
struct Owned(HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the handle was returned open by `OpenProcess` or `CreateToolhelp32Snapshot` and is closed only here.
        unsafe { CloseHandle(self.0) };
    }
}

pub(crate) fn resident_bytes(pid: u32) -> Option<u64> {
    // SAFETY: `OpenProcess` takes plain values and returns a new handle or null; it reads no memory of ours.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let handle = Owned(handle);
    let mut code = 0u32;
    // SAFETY: `handle` is open with PROCESS_QUERY_LIMITED_INFORMATION, which the call needs; `code` is a valid u32.
    let answered = unsafe { GetExitCodeProcess(handle.0, &raw mut code) };
    // A process that ended while someone still holds its handle keeps its pid; it is not counted.
    if answered == 0 || code != STILL_ACTIVE.cast_unsigned() {
        return None;
    }
    let size = u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS>()).ok()?;
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: size,
        PageFaultCount: 0,
        PeakWorkingSetSize: 0,
        WorkingSetSize: 0,
        QuotaPeakPagedPoolUsage: 0,
        QuotaPagedPoolUsage: 0,
        QuotaPeakNonPagedPoolUsage: 0,
        QuotaNonPagedPoolUsage: 0,
        PagefileUsage: 0,
        PeakPagefileUsage: 0,
    };
    // SAFETY: `handle` is open with PROCESS_QUERY_LIMITED_INFORMATION, enough for this call since Windows 7;
    // `counters` is a writable PROCESS_MEMORY_COUNTERS whose size is passed in `cb` and as the length.
    let read = unsafe { K32GetProcessMemoryInfo(handle.0, &raw mut counters, size) };
    if read == 0 {
        return None;
    }
    u64::try_from(counters.WorkingSetSize).ok()
}

pub(crate) fn process_table() -> Vec<ProcessInfo> {
    // SAFETY: the call takes plain values and returns a new snapshot handle or INVALID_HANDLE_VALUE.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
        return Vec::new();
    }
    let snapshot = Owned(snapshot);
    let mut table = Vec::new();
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).unwrap_or(u32::MAX),
        ..PROCESSENTRY32W::default()
    };
    // SAFETY: `snapshot` is an open process snapshot and `entry` a writable PROCESSENTRY32W with `dwSize` set.
    let mut more = unsafe { Process32FirstW(snapshot.0, &raw mut entry) } != 0;
    while more {
        table.push(info_of(&entry));
        // SAFETY: as above; the call overwrites `entry` with the next process.
        more = unsafe { Process32NextW(snapshot.0, &raw mut entry) } != 0;
    }
    table
}

fn info_of(entry: &PROCESSENTRY32W) -> ProcessInfo {
    let name = &entry.szExeFile;
    let len = name
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(name.len());
    ProcessInfo {
        pid: entry.th32ProcessID,
        // The idle process (pid 0) names itself as its parent.
        parent: (entry.th32ProcessID != 0).then_some(entry.th32ParentProcessID),
        name: String::from_utf16_lossy(&name[..len]),
    }
}
