//! Test-only: stops `service run` once the test process that started it is gone, so a test killed before its sandbox
//! drops leaves no service behind. Inert unless `MASSORCA_TEST_PARENT_PID` is set, which only the CLI test harness does.

use std::time::Duration;

use massorca_server::ShutdownHandle;

/// The environment variable that names the process a service watches.
pub const PARENT_PID_ENV: &str = "MASSORCA_TEST_PARENT_PID";
const POLL: Duration = Duration::from_secs(1);

/// The watched process id, when the variable is set to one.
pub fn parent_pid() -> Option<u32> {
    std::env::var(PARENT_PID_ENV).ok()?.trim().parse().ok()
}

/// Requests shutdown once process `pid` is gone. Must run on the service's runtime.
pub fn stop_with_parent(pid: u32, shutdown: ShutdownHandle) {
    tokio::spawn(async move {
        while process_is_alive(pid) {
            tokio::time::sleep(POLL).await;
        }
        tracing::warn!(
            parent = pid,
            "the test process that started this service is gone; stopping"
        );
        shutdown.shutdown();
    });
}

/// A zombie counts as alive until its parent reaps it.
#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    use rustix::io::Errno;
    use rustix::process::{Pid, test_kill_process};

    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return false;
    };
    match test_kill_process(pid) {
        Ok(()) => true,
        // Another user's process still exists; signalling it is only refused.
        Err(error) => error == Errno::PERM,
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn process_is_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, GetLastError, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };

    // SAFETY: OpenProcess takes plain values and returns a handle or null; a handle is closed below, once.
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        // No such process, or one of another identity that may not be opened: only the latter is alive.
        // SAFETY: GetLastError only reads the calling thread's last error.
        return unsafe { GetLastError() } == ERROR_ACCESS_DENIED;
    }
    // SAFETY: the handle is valid here and is closed exactly once, after the wait.
    let alive = unsafe { WaitForSingleObject(handle, 0) } == WAIT_TIMEOUT;
    // SAFETY: the same handle, closed once.
    unsafe { CloseHandle(handle) };
    alive
}
