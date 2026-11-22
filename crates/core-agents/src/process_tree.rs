//! Starts a program as the root of its own process tree, and stops the whole tree at once (decision 0059). A check or a
//! sign-out that hangs must not keep its children's output pipes open, or the account's gate and its output stay held.
//!
//! Windows: the child starts suspended, is put in a kill-on-close job, and is then resumed, so nothing it starts escapes
//! the job. Closing the job ends every process in it. Unix: the child leads its own process group, and stopping sends
//! SIGKILL to the group. A group can be signalled after its leader was reaped, while members remain.

#[cfg(unix)]
#[allow(unsafe_code)]
mod imp {
    use std::io;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command};

    /// The process group of a started program, led by the program itself.
    pub(crate) struct Tree {
        group: Option<i32>,
    }

    pub(crate) fn spawn(command: &mut Command) -> io::Result<(Child, Tree)> {
        command.process_group(0);
        let child = command.spawn()?;
        let group = i32::try_from(child.id()).map_err(io::Error::other)?;
        Ok((child, Tree { group: Some(group) }))
    }

    impl Tree {
        /// Sends SIGKILL to every process of the group. Once is enough; a second call does nothing.
        pub(crate) fn stop(&mut self) {
            if let Some(group) = self.group.take() {
                // SAFETY: `killpg` sends one signal to a process group and reads or writes no memory. The group id is
                // the pid of the program this module started, which leads it.
                let _ = unsafe { libc::killpg(group, libc::SIGKILL) };
            }
        }
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod imp {
    use std::ffi::c_void;
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::process::{Child, Command};

    use massorca_process::CREATE_SUSPENDED;
    use win32job::{ExtendedLimitInfo, Job};

    // One system call resumes every thread of a process. On 32-bit x86 a raw-dylib import is decorated unless told
    // otherwise, and ntdll exports this one bare (the same import core-git uses for its git processes).
    #[cfg_attr(
        target_arch = "x86",
        link(name = "ntdll", kind = "raw-dylib", import_name_type = "undecorated")
    )]
    #[cfg_attr(not(target_arch = "x86"), link(name = "ntdll", kind = "raw-dylib"))]
    unsafe extern "system" {
        fn NtResumeProcess(process: *mut c_void) -> i32;
    }

    /// The kill-on-close job of a started program's tree. Dropping it ends every process in the job.
    pub(crate) struct Tree {
        job: Option<Job>,
    }

    pub(crate) fn spawn(command: &mut Command) -> io::Result<(Child, Tree)> {
        let job = Job::create_with_limit_info(ExtendedLimitInfo::new().limit_kill_on_job_close())
            .map_err(|error| io::Error::other(format!("{error:?}")))?;
        // The helper keeps CREATE_NO_WINDOW: the service has no console, so a console program would open a window.
        massorca_process::creation_flags(command, CREATE_SUSPENDED);
        let mut child = command.spawn()?;
        if let Err(error) = job.assign_process(child.as_raw_handle() as isize) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other(format!("{error:?}")));
        }
        // SAFETY: the handle is the child's own process handle, open for as long as `child` lives; std opens it with
        // PROCESS_ALL_ACCESS, which includes the PROCESS_SUSPEND_RESUME right the call needs.
        let status = unsafe { NtResumeProcess(child.as_raw_handle()) };
        if status < 0 {
            drop(job);
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other(format!(
                "the suspended program could not be resumed (NTSTATUS {status:#x})"
            )));
        }
        Ok((child, Tree { job: Some(job) }))
    }

    impl Tree {
        /// Ends every process of the job by closing it. Once is enough; a second call does nothing.
        pub(crate) fn stop(&mut self) {
            self.job.take();
        }
    }
}

pub(crate) use imp::spawn;
