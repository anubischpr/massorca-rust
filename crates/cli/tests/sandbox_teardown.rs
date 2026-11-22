//! Dropping a `Sandbox` leaves no service behind, even one that was still starting when the sandbox dropped.
#![cfg(any(windows, target_os = "linux"))]

mod harness;

use std::fs::{self, OpenOptions};
use std::path::Path;
use std::process::{Command, Stdio};

use harness::{Sandbox, eventually};

#[test]
fn a_service_still_starting_when_the_sandbox_drops_does_not_outlive_it() {
    let sandbox = Sandbox::new();
    let profile = format!("teardown-{}", std::process::id());
    let dir = sandbox.profile_dir(&profile);
    fs::create_dir_all(&dir).expect("the profile directory");
    // Plays a predecessor that is still draining: an autostarted service waits for this lock without answering.
    let lock_path = dir.join("service.lock");
    let gate = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .expect("the lock file opens");
    gate.try_lock().expect("the lock is free");

    let mut client = sandbox
        .command(&profile)
        .args(["service", "info"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the client starts");
    eventually("the autostarted service to open the lock file", || {
        opened_lock(&dir, &profile).then_some(())
    });
    let _ = client.kill();
    let _ = client.wait();
    // `service stop` looks for the lock at its path, so the waiting service is invisible to it once the file moves.
    fs::rename(&lock_path, sandbox.home().join("gate.lock")).expect("the lock file moves");

    drop(sandbox);
    // A service still waiting now takes the lock and would serve until killed.
    drop(gate);

    let left = service_processes(&profile);
    for pid in &left {
        kill(*pid);
    }
    assert!(left.is_empty(), "services outlived their sandbox: {left:?}");
}

/// The `massorca service run --profile <profile>` processes on this machine.
#[cfg(target_os = "linux")]
fn service_processes(profile: &str) -> Vec<u32> {
    let wanted = ["service", "run", "--profile", profile];
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| {
            let Ok(cmdline) = fs::read(format!("/proc/{pid}/cmdline")) else {
                return false;
            };
            let args: Vec<&[u8]> = cmdline.split(|byte| *byte == 0).collect();
            args.windows(wanted.len()).any(|window| {
                window
                    .iter()
                    .zip(wanted)
                    .all(|(arg, want)| *arg == want.as_bytes())
            })
        })
        .collect()
}

/// Whether a service of `profile` holds its lock file open, so moving the file can no longer race its opening it.
#[cfg(target_os = "linux")]
fn opened_lock(_dir: &Path, profile: &str) -> bool {
    service_processes(profile).into_iter().any(|pid| {
        fs::read_dir(format!("/proc/{pid}/fd")).is_ok_and(|fds| {
            fds.filter_map(Result::ok).any(|fd| {
                fs::read_link(fd.path()).is_ok_and(|target| target.ends_with("service.lock"))
            })
        })
    })
}

#[cfg(target_os = "linux")]
fn kill(pid: u32) {
    let _ = Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .status();
}

/// The `massorca service run --profile <profile>` processes on this machine.
#[cfg(windows)]
fn service_processes(profile: &str) -> Vec<u32> {
    let script = format!(
        "Get-CimInstance Win32_Process -Filter \"Name='massorca.exe'\" | \
         Where-Object {{ $_.CommandLine -match 'service run --profile {profile}(\\s|$)' }} | \
         ForEach-Object {{ $_.ProcessId }}"
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(Stdio::null())
        .output()
        .expect("powershell runs");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

/// Windows cannot show which files a process holds without `unsafe`. The service creates its log file just before it
/// opens the lock file, and the poll interval covers that gap.
#[cfg(windows)]
fn opened_lock(dir: &Path, _profile: &str) -> bool {
    fs::read_dir(dir.join("logs")).is_ok_and(|mut files| files.next().is_some())
}

#[cfg(windows)]
fn kill(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}
