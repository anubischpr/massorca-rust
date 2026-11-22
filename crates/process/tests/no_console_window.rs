//! A console child of a process with no console, as the detached service is, gets no console window.
#![cfg(windows)]

use std::process::Stdio;

/// Which part of the test this process plays: unset for the test itself, then the console-less parent, then the child.
const ROLE: &str = "MASSORCA_PROCESS_TEST_ROLE";
const TEST: &str = "a_child_of_a_parent_with_no_console_gets_no_console_window";

/// Runs this same test in a new process playing `role`, and reports whether it passed.
fn run_as(role: &str, flags: u32) -> bool {
    let exe = std::env::current_exe().expect("the test binary");
    let mut command = massorca_process::command(exe);
    command
        .args([TEST, "--exact", "--test-threads=1"])
        .env(ROLE, role)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    massorca_process::creation_flags(&mut command, flags);
    command.status().expect("the test binary starts").success()
}

#[allow(unsafe_code)]
fn console_window_is_null() -> bool {
    // SAFETY: GetConsoleWindow takes no arguments and only reads this process's console state.
    unsafe { windows_sys::Win32::System::Console::GetConsoleWindow() }.is_null()
}

#[test]
fn a_child_of_a_parent_with_no_console_gets_no_console_window() {
    match std::env::var(ROLE).as_deref() {
        // The child: a console program that was given a console; it must have no window.
        Ok("child") => assert!(console_window_is_null(), "the child has a console window"),
        // The parent: no console of its own, like the detached service; its child gets a new console.
        Ok("parent") => {
            assert!(console_window_is_null(), "a detached parent has no console");
            assert!(run_as("child", 0), "the child has a console window");
        }
        _ => assert!(
            run_as("parent", massorca_process::DETACHED_PROCESS),
            "a child of a parent with no console got a console window"
        ),
    }
}
