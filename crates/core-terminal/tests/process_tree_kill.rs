//! Ending a terminal's whole process tree (decision 0031), against real shells.
//! Every wait is bounded, so a failure reports instead of hanging the suite.

mod support;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

use massorca_core_terminal::process_tree::ProcessTree;
use massorca_core_terminal::pty::{ExitInfo, PtyMaster, SpawnSpec, Spawned, spawn};
use massorca_core_terminal::shell::{EnvSnapshot, resolve_program};
use support::{Output, TIMEOUT, capture, shell_program, spec, terminal_env, wait_for_exit};

/// How long a kill may take before the test calls it a failure (0033 kills within 5 s).
const KILL_BOUND: Duration = Duration::from_secs(30);

// Compile-time check that the tree handle may be shared between threads.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ProcessTree>();
};

/// Runs in the temp dir, as every tree test does.
fn tree_spec(program: PathBuf, args: &[&str]) -> SpawnSpec {
    spec(program, args, &std::env::temp_dir(), terminal_env())
}

/// A running terminal. Dropping it kills the tree, so a failed assertion leaves nothing behind.
struct Running {
    master: Arc<PtyMaster>,
    output: Output,
    exit: Receiver<ExitInfo>,
    tree: ProcessTree,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.tree.kill(None);
        self.master.close();
    }
}

fn start(spec: &SpawnSpec) -> Running {
    let Spawned {
        master,
        reader,
        child,
        tree,
    } = spawn(spec).expect("spawn the program in a pseudo-terminal");
    let master = Arc::new(master);
    let output = capture(reader, Arc::clone(&master));
    Running {
        exit: wait_for_exit(child),
        master,
        output,
        tree,
    }
}

/// Waits until `members` reports at least `wanted` live processes.
fn wait_for_members(tree: &ProcessTree, wanted: usize) -> Vec<u32> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let members = tree.members();
        if members.len() >= wanted {
            return members;
        }
        assert!(Instant::now() < deadline, "only {members:?} started");
        thread::sleep(Duration::from_millis(20));
    }
}

/// Waits until the tree lists no live process.
fn wait_for_no_members(tree: &ProcessTree) {
    let deadline = Instant::now() + KILL_BOUND;
    while !tree.members().is_empty() {
        assert!(
            Instant::now() < deadline,
            "members still listed after the kill"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// Waits until every pid in `pids` is gone, within the kill bound.
fn assert_all_gone(pids: &[u32]) {
    let deadline = Instant::now() + KILL_BOUND;
    for &pid in pids {
        while !is_gone(pid) {
            assert!(Instant::now() < deadline, "process {pid} outlived the kill");
            thread::sleep(Duration::from_millis(50));
        }
    }
}

#[cfg(target_os = "linux")]
fn is_gone(pid: u32) -> bool {
    // A zombie has ended; its parent just has not reaped it yet.
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => true,
        Ok(stat) => stat
            .rsplit(')')
            .next()
            .and_then(|rest| rest.split_whitespace().next())
            .is_some_and(|state| state == "Z" || state == "X"),
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn is_gone(pid: u32) -> bool {
    use rustix::io::Errno;
    use rustix::process::{Pid, test_kill_process};

    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return true;
    };
    test_kill_process(pid).is_err_and(|error| error == Errno::SRCH)
}

#[cfg(windows)]
fn is_gone(pid: u32) -> bool {
    use std::process::Stdio;

    let output = massorca_process::command("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .stdin(Stdio::null())
        .output()
        .expect("run tasklist");
    !String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
}

/// The pid a `child=<pid>` line names.
fn child_pid(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("child="))
        .map(|pid| pid.trim().to_owned())
        .filter(|pid| !pid.is_empty() && pid.bytes().all(|byte| byte.is_ascii_digit()))
}

#[test]
fn kill_ends_the_shell_and_the_processes_it_started() {
    let args: &[&str] = if cfg!(windows) {
        &[
            "/d",
            "/c",
            "start /b ping -n 120 127.0.0.1 >nul & ping -n 120 127.0.0.1 >nul",
        ]
    } else {
        &["-c", "sleep 120 & echo child=$!; sleep 120"]
    };
    let running = start(&tree_spec(shell_program(), args));

    if cfg!(unix) {
        running
            .output
            .wait_for_value("the background child", child_pid);
    }
    let before = wait_for_members(&running.tree, 2);

    let killed_at = Instant::now();
    running.tree.kill(None);
    let exit = running
        .exit
        .recv_timeout(KILL_BOUND)
        .expect("the shell exits within the kill bound");
    assert!(killed_at.elapsed() < KILL_BOUND, "{exit:?}");
    assert_all_gone(&before);
    wait_for_no_members(&running.tree);
}

#[cfg(unix)]
#[test]
fn hangup_ends_an_interactive_shell() {
    let running = start(&tree_spec(shell_program(), &["-i"]));
    wait_for_members(&running.tree, 1);

    running.tree.hangup(None);

    let exit = running
        .exit
        .recv_timeout(KILL_BOUND)
        .expect("the interactive shell exits after SIGHUP");
    assert!(exit.code.is_some() || exit.signal.is_some(), "{exit:?}");
}

#[cfg(unix)]
#[test]
fn kill_reaches_a_job_that_runs_in_its_own_process_group() {
    let bash = resolve_program("bash", &EnvSnapshot::current()).expect("bash is on PATH");
    let running = start(&tree_spec(
        bash,
        &["-c", "set -m; sleep 120 & echo child=$!; sleep 120"],
    ));
    let child = running
        .output
        .wait_for_value("the background job", child_pid);
    let child_pid_value: i32 = child.parse().expect("a pid");
    let shell_pid =
        i32::try_from(running.master.pid().expect("the shell has a pid")).expect("pid fits");

    let job_pid = rustix::process::Pid::from_raw(child_pid_value).expect("a valid pid");
    let group = rustix::process::getpgid(Some(job_pid)).expect("the job has a process group");
    assert_ne!(
        group.as_raw_nonzero().get(),
        shell_pid,
        "the job must lead its own group for this test to mean anything"
    );

    let before = running.tree.members();
    running.tree.kill(None);
    running
        .exit
        .recv_timeout(KILL_BOUND)
        .expect("the shell exits within the kill bound");
    assert_all_gone(&before);
    assert!(is_gone(u32::try_from(child_pid_value).expect("pid")));
}

#[test]
fn kill_is_idempotent_and_leaves_no_members() {
    let program = if cfg!(windows) {
        shell_program()
    } else {
        resolve_program("sleep", &EnvSnapshot::current()).expect("sleep is on PATH")
    };
    let args: &[&str] = if cfg!(windows) {
        &["/d", "/c", "ping -n 120 127.0.0.1 >nul"]
    } else {
        &["120"]
    };
    let running = start(&tree_spec(program, args));
    let before = wait_for_members(&running.tree, 1);

    running.tree.kill(None);
    running.tree.kill(None);

    running
        .exit
        .recv_timeout(KILL_BOUND)
        .expect("the program exits within the kill bound");
    assert_all_gone(&before);
    wait_for_no_members(&running.tree);
}
