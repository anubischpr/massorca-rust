//! Starts a child suspended, puts it in a kill-on-close job, then lets it run, so nothing it starts can escape
//! the job. Decision 0007 allows `unsafe` only in named Windows modules such as this one.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};

use win32job::{ExtendedLimitInfo, Job};
use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;

// One system call resumes every thread of a process; enumerating threads with a Toolhelp snapshot instead
// costs a scan of every thread on the machine per spawn, which doubled the time of a git-heavy test run.
#[link(name = "ntdll", kind = "raw-dylib")]
unsafe extern "system" {
    fn NtResumeProcess(process: *mut c_void) -> i32;
}

/// Spawns `command` with `flags` inside a new kill-on-close job. Assigning a running child left a window in which
/// a busy machine let it start a hook outside the job, and the hook then survived the kill. Without a job the
/// child runs alone, and only it can be killed.
pub(crate) fn spawn_in_job(command: &mut Command, flags: u32) -> io::Result<(Child, Option<Job>)> {
    let Ok(job) = Job::create_with_limit_info(ExtendedLimitInfo::new().limit_kill_on_job_close())
    else {
        command.creation_flags(flags);
        return Ok((command.spawn()?, None));
    };
    command.creation_flags(flags | CREATE_SUSPENDED);
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
