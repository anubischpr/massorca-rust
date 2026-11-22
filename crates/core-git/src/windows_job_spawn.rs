//! Starts a child suspended, puts it in a kill-on-close job, then lets it run, so nothing it starts can escape
//! the job. Decision 0007 allows `unsafe` only in named Windows modules such as this one.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use win32job::{ExtendedLimitInfo, Job};
use windows_sys::Win32::System::JobObjects::TerminateJobObject;
use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;

// One system call resumes every thread of a process; enumerating threads with a Toolhelp snapshot instead
// costs a scan of every thread on the machine per spawn, which doubled the time of a git-heavy test run.
// On 32-bit x86 a raw-dylib import is decorated (`_NtResumeProcess@4`) unless told otherwise; ntdll exports it bare.
#[cfg_attr(
    target_arch = "x86",
    link(name = "ntdll", kind = "raw-dylib", import_name_type = "undecorated")
)]
#[cfg_attr(not(target_arch = "x86"), link(name = "ntdll", kind = "raw-dylib"))]
unsafe extern "system" {
    fn NtResumeProcess(process: *mut c_void) -> i32;
}

/// Spawns `command` with `flags` inside a new kill-on-close job. Assigning a running child left a window in which
/// a busy machine let it start a hook outside the job, and the hook then survived the kill. Without a job the
/// child runs alone, and only it can be killed.
pub(crate) fn spawn_in_job(command: &mut Command, flags: u32) -> io::Result<(Child, Option<Job>)> {
    let Ok(job) = Job::create_with_limit_info(ExtendedLimitInfo::new().limit_kill_on_job_close())
    else {
        massorca_process::creation_flags(command, flags);
        return Ok((command.spawn()?, None));
    };
    massorca_process::creation_flags(command, flags | CREATE_SUSPENDED);
    let mut child = command.spawn()?;
    let job = job
        .assign_process(child.as_raw_handle() as isize)
        .is_ok()
        .then_some(job);
    // SAFETY: the handle is the child's own process handle, open for as long as `child` lives; std opens it with
    // PROCESS_ALL_ACCESS, which includes the PROCESS_SUSPEND_RESUME right the call needs.
    let status = unsafe { NtResumeProcess(child.as_raw_handle()) };
    if status < 0 {
        let _ = child.kill();
        let _ = child.wait();
        return Err(io::Error::other(format!(
            "the suspended git process could not be resumed (NTSTATUS {status:#x})"
        )));
    }
    Ok((child, job))
}

/// Terminates every process in `job` and waits, up to `wait`, until none is left. Closing the job alone also kills
/// them, but returns before they exit.
pub(crate) fn terminate_and_wait(job: &Job, wait: Duration) {
    // SAFETY: `job.handle()` is the job's open handle, valid for as long as `job` lives.
    unsafe { TerminateJobObject(job.handle() as *mut c_void, 1) };
    let deadline = Instant::now() + wait;
    while job
        .query_basic_and_io_accounting_info()
        .is_ok_and(|info| info.active_processes > 0)
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
}
