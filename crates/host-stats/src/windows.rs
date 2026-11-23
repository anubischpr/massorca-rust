//! Windows: the working set through `K32GetProcessMemoryInfo`, the table through a Toolhelp32 snapshot.
//! Decision 0007 allows `unsafe` only in named platform modules such as this one.
#![allow(unsafe_code)]

use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::ptr;

use win32job::Job;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_MORE_DATA, ERROR_NO_MORE_FILES, GetLastError,
    HANDLE, INVALID_HANDLE_VALUE, NO_ERROR, STILL_ACTIVE,
};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
};
use windows_sys::Win32::Networking::WinSock::{ADDRESS_FAMILY, AF_INET, AF_INET6};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::JobObjects::{
    IsProcessInJob, JOBOBJECT_BASIC_PROCESS_ID_LIST, JobObjectBasicProcessIdList,
    QueryInformationJobObject,
};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::{
    Listening, ListeningSocket, METRIC, MemorySample, ProcessInfo, ProcessMemory, ProcessTable,
};

/// Room for this many pids in the first job query; a larger job is queried again with the size it reports.
const JOB_IDS: usize = 1024;
/// Bytes before the pid list in JOBOBJECT_BASIC_PROCESS_ID_LIST: two u32 counts.
const JOB_LIST_HEADER: usize = 8;
/// Queries before a job that keeps growing is given up on.
const JOB_ATTEMPTS: usize = 4;
/// Queries before a TCP owner table that keeps growing is given up on.
const TABLE_ATTEMPTS: usize = 4;

/// A handle this module opened, closed when dropped.
struct Owned(HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the handle was returned open by `OpenProcess` or `CreateToolhelp32Snapshot` and is closed only here.
        unsafe { CloseHandle(self.0) };
    }
}

/// The raw handle of a job; `job` keeps it open while it is borrowed.
fn job_handle(job: &Job) -> HANDLE {
    job.handle() as HANDLE
}

/// Opens `pid` for queries while it still runs. A process that ended while someone still holds its handle keeps its
/// pid; it is not opened.
fn open_running(pid: u32) -> Option<Owned> {
    // SAFETY: `OpenProcess` takes plain values and returns a new handle or null; it reads no memory of ours.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let handle = Owned(handle);
    let mut code = 0u32;
    // SAFETY: `handle` is open with PROCESS_QUERY_LIMITED_INFORMATION, which the call needs; `code` is a valid u32.
    let answered = unsafe { GetExitCodeProcess(handle.0, &raw mut code) };
    (answered != 0 && code == STILL_ACTIVE.cast_unsigned()).then_some(handle)
}

fn working_set(process: &Owned) -> Option<u64> {
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
    // SAFETY: `process` is open with PROCESS_QUERY_LIMITED_INFORMATION, enough for this call since Windows 7;
    // `counters` is a writable PROCESS_MEMORY_COUNTERS whose size is passed in `cb` and as the length.
    let read = unsafe { K32GetProcessMemoryInfo(process.0, &raw mut counters, size) };
    if read == 0 {
        return None;
    }
    u64::try_from(counters.WorkingSetSize).ok()
}

pub(crate) fn resident_bytes(pid: u32) -> Option<u64> {
    working_set(&open_running(pid)?)
}

/// Whether the opened process is in `job`. Checked on the open handle, so a pid reused since the job was listed is not
/// read as the job's.
fn in_job(process: &Owned, job: &Job) -> bool {
    let mut member = 0;
    // SAFETY: `process` is open with PROCESS_QUERY_LIMITED_INFORMATION, which the call needs; the job handle stays open
    // while `job` is borrowed; `member` is a valid BOOL.
    let answered = unsafe { IsProcessInJob(process.0, job_handle(job), &raw mut member) };
    answered != 0 && member != 0
}

/// The memory of `pid` while it still runs in `job`.
fn memory_in_job(pid: u32, job: &Job) -> Option<u64> {
    let process = open_running(pid)?;
    if !in_job(&process, job) {
        return None;
    }
    working_set(&process)
}

pub(crate) fn job_members(job: &Job) -> Option<Vec<u32>> {
    job_ids(job, JOB_IDS)
}

pub(crate) fn sample_job(job: &Job) -> Option<MemorySample> {
    let mut pids = job_members(job)?;
    pids.sort_unstable();
    pids.dedup();
    let mut processes = Vec::new();
    let mut unreadable = 0u32;
    for pid in pids {
        match memory_in_job(pid, job) {
            Some(bytes) => processes.push(ProcessMemory { pid, bytes }),
            None => unreadable = unreadable.saturating_add(1),
        }
    }
    Some(MemorySample {
        metric: METRIC,
        processes,
        unreadable,
    })
}

/// The job's pids, starting with room for `capacity` and growing to the count the job reports. `None` when the job
/// cannot be queried.
fn job_ids(job: &Job, capacity: usize) -> Option<Vec<u32>> {
    let header = JOB_LIST_HEADER.div_ceil(size_of::<usize>());
    let mut capacity = capacity.max(1);
    for _ in 0..JOB_ATTEMPTS {
        let mut buffer = vec![0usize; header + capacity];
        let bytes = u32::try_from(buffer.len() * size_of::<usize>()).ok()?;
        let list = buffer
            .as_mut_ptr()
            .cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>();
        // SAFETY: the job handle stays open while `job` is borrowed; `list` points to `bytes` writable, zeroed bytes
        // aligned for usize, the structure's alignment; the call writes at most `bytes` bytes.
        let answered = unsafe {
            QueryInformationJobObject(
                job_handle(job),
                JobObjectBasicProcessIdList,
                list.cast(),
                bytes,
                ptr::null_mut(),
            )
        };
        // SAFETY: the two counts lie inside the buffer and are initialized (zeroed, or written by the call).
        let (assigned, listed) = unsafe {
            (
                ptr::addr_of!((*list).NumberOfAssignedProcesses).read(),
                ptr::addr_of!((*list).NumberOfProcessIdsInList).read(),
            )
        };
        if answered != 0 && listed >= assigned {
            let listed = usize::try_from(listed).ok()?.min(capacity);
            return Some(
                buffer[header..header + listed]
                    .iter()
                    .filter_map(|pid| u32::try_from(*pid).ok())
                    .collect(),
            );
        }
        // SAFETY: reads this thread's last error code; no memory of ours is touched.
        if answered == 0 && unsafe { GetLastError() } != ERROR_MORE_DATA {
            return None;
        }
        let reported = usize::try_from(assigned).unwrap_or(0);
        capacity = reported.saturating_add(64).max(capacity.saturating_mul(2));
    }
    None
}

pub(crate) fn process_table() -> ProcessTable {
    // SAFETY: the call takes plain values and returns a new snapshot handle or INVALID_HANDLE_VALUE.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
        return ProcessTable {
            processes: Vec::new(),
            complete: false,
        };
    }
    let snapshot = Owned(snapshot);
    let mut processes = Vec::new();
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).unwrap_or(u32::MAX),
        ..PROCESSENTRY32W::default()
    };
    // SAFETY: `snapshot` is an open process snapshot and `entry` a writable PROCESSENTRY32W with `dwSize` set.
    let mut more = unsafe { Process32FirstW(snapshot.0, &raw mut entry) } != 0;
    while more {
        processes.push(info_of(&entry));
        // SAFETY: as above; the call overwrites `entry` with the next process.
        more = unsafe { Process32NextW(snapshot.0, &raw mut entry) } != 0;
    }
    // The walk ends normally only when the snapshot has no more processes; any other failure leaves the table partial.
    // SAFETY: reads this thread's last error code; no memory of ours is touched.
    let complete = unsafe { GetLastError() } == ERROR_NO_MORE_FILES;
    ProcessTable {
        processes,
        complete,
    }
}

/// The TCP listeners of `pids`, IPv4 and IPv6, from the system's owner tables. A row's address and port are in network
/// byte order. A listener names only its owner's pid, so the caller confirms that the owner still belongs to the
/// terminal (`job_still_holds`) after this read. `None` when a table cannot be read.
pub(crate) fn listening_sockets(_table: &[ProcessInfo], pids: &BTreeSet<u32>) -> Option<Listening> {
    let v4 = owner_table(AF_INET)?;
    let v6 = owner_table(AF_INET6)?;
    let mut sockets = Vec::new();
    for row in owner_rows::<MIB_TCPROW_OWNER_PID>(&v4) {
        if pids.contains(&row.dwOwningPid) {
            sockets.push(ListeningSocket {
                pid: row.dwOwningPid,
                address: IpAddr::V4(Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes())),
                port: port_of(row.dwLocalPort),
            });
        }
    }
    for row in owner_rows::<MIB_TCP6ROW_OWNER_PID>(&v6) {
        if pids.contains(&row.dwOwningPid) {
            sockets.push(ListeningSocket {
                pid: row.dwOwningPid,
                address: IpAddr::V6(Ipv6Addr::from(row.ucLocalAddr)),
                port: port_of(row.dwLocalPort),
            });
        }
    }
    Some(Listening {
        sockets,
        unreadable: BTreeSet::new(),
    })
}

/// The system's TCP owner table of one family, listing only LISTEN sockets with their owners' pids. It is read into
/// words, so its rows are aligned for their u32 fields, and asked again with the size the system reports while it grows.
/// Returns the buffer and the bytes the table fills. `None` when it cannot be read.
fn owner_table(family: ADDRESS_FAMILY) -> Option<(Vec<u64>, usize)> {
    let mut size = 0u32;
    for _ in 0..TABLE_ATTEMPTS {
        let words = usize::try_from(size).ok()?.div_ceil(size_of::<u64>());
        let mut buffer = vec![0u64; words];
        let mut needed = u32::try_from(words * size_of::<u64>()).ok()?;
        // SAFETY: `buffer` holds `needed` writable bytes, aligned for the table's u32 fields; the call writes at most
        // `needed` bytes and sets `needed` to the size the table needs.
        let status = unsafe {
            GetExtendedTcpTable(
                buffer.as_mut_ptr().cast(),
                &raw mut needed,
                0,
                u32::from(family),
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if status == NO_ERROR {
            return Some((buffer, usize::try_from(needed).ok()?));
        }
        if status != ERROR_INSUFFICIENT_BUFFER {
            return None;
        }
        size = needed;
    }
    None
}

/// The rows of an owner table: its entry count (a u32), then the rows. Only the bytes the call filled are read, and a
/// count that claims more rows than fit is cut to those that fit. The rows hold only integers, so any bytes are a row.
fn owner_rows<R>((buffer, filled): &(Vec<u64>, usize)) -> Vec<R> {
    let header = size_of::<u32>();
    let row = size_of::<R>();
    let bytes = (*filled).min(buffer.len() * size_of::<u64>());
    if bytes < header {
        return Vec::new();
    }
    let base = buffer.as_ptr().cast::<u8>();
    // SAFETY: the first `header` bytes of the buffer are initialized and filled by the call; the read needs no alignment.
    let count = unsafe { ptr::read_unaligned(base.cast::<u32>()) };
    let fit = (bytes - header) / row;
    let count = usize::try_from(count).unwrap_or(usize::MAX).min(fit);
    (0..count)
        .map(|index| {
            // SAFETY: row `index` lies within the `bytes` the call filled (`fit` rows fit); the read needs no alignment.
            unsafe { ptr::read_unaligned(base.add(header + index * row).cast::<R>()) }
        })
        .collect()
}

/// A row's port: its low 16 bits hold the port in network byte order.
fn port_of(field: u32) -> u16 {
    u16::from_be((field & 0xFFFF) as u16)
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
        session: None,
        start: None,
    }
}

#[cfg(test)]
mod tests {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;

    use win32job::{ExtendedLimitInfo, Job};

    use super::{job_ids, memory_in_job, resident_bytes};

    fn job() -> Job {
        Job::create_with_limit_info(ExtendedLimitInfo::new().limit_kill_on_job_close())
            .expect("a job")
    }

    /// A child that waits, killed through its own handle when dropped.
    struct Waiter(Child);

    impl Drop for Waiter {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn waiter_in(job: &Job) -> Waiter {
        // An interactive cmd waits on its open stdin and starts no child, so nothing it runs can be outside the job.
        let child = massorca_process::command("cmd.exe")
            .args(["/d", "/q", "/k"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("cmd starts");
        let waiter = Waiter(child);
        job.assign_process(waiter.0.as_raw_handle() as isize)
            .expect("the child joins the job");
        waiter
    }

    #[test]
    fn a_process_outside_the_job_is_not_read_as_its_member() {
        let job = job();
        let own = std::process::id();
        assert!(resident_bytes(own).is_some());
        assert_eq!(memory_in_job(own, &job), None);
    }

    #[test]
    fn a_job_larger_than_the_first_buffer_is_listed_whole() {
        let job = job();
        let first = waiter_in(&job);
        let second = waiter_in(&job);
        let listed = job_ids(&job, 1).expect("the job is listed");
        assert!(listed.contains(&first.0.id()), "{listed:?}");
        assert!(listed.contains(&second.0.id()), "{listed:?}");
    }
}
