//! Starts `massorca service run` detached from this process (ADR 0003), so the service outlives the client that started it.

use std::process::{Child, Command, Stdio};

use massorca_profile::Profile;

use crate::error::ClientError;
use crate::options::Autostart;

pub(crate) fn service_run(profile: &Profile, autostart: &Autostart) -> Result<(), ClientError> {
    // The profile directory exists before the service writes anything into it.
    std::fs::create_dir_all(profile.dir()).map_err(|error| {
        ClientError::StartFailed(format!("{}: {error}", profile.dir().display()))
    })?;
    let mut command = service_command(profile, autostart);
    detach(&mut command);
    keep_handles_private();
    let child = command.spawn().map_err(|error| {
        ClientError::StartFailed(format!("{}: {error}", autostart.exe.display()))
    })?;
    reap_when_it_exits(child);
    Ok(())
}

/// Waits for the started service on a thread of its own. Until it exits the service is this process's child, and an
/// unwaited child stays a zombie for as long as the client lives. Detaching is process groups only: `setsid` would need a
/// second fork, which takes `unsafe`, and the workspace forbids it.
#[cfg(unix)]
fn reap_when_it_exits(mut child: Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}

/// Windows releases the process when its handle closes, so there is nothing to wait for.
#[cfg(windows)]
fn reap_when_it_exits(child: Child) {
    drop(child);
}

/// The command that runs the service. It starts in the profile directory, never in the caller's
/// working directory: a service that inherits a caller's directory resolves relative paths
/// against it, so two clients in different directories would share one service's view of paths.
fn service_command(profile: &Profile, autostart: &Autostart) -> Command {
    let mut command = Command::new(&autostart.exe);
    command
        .args(["service", "run", "--profile", profile.name()])
        .args(&autostart.args_extra)
        .env("MASSORCA_HOME", profile.home())
        .current_dir(profile.dir())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// Stops the detached service from inheriting this process's standard pipes, so a caller reading them to end-of-file is not held open.
#[cfg(windows)]
fn keep_handles_private() {
    crate::windows_handle_inheritance::clear_standard_handle_inheritance();
}

#[cfg(unix)]
fn keep_handles_private() {}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use massorca_profile::Profile;

    use super::service_command;
    use crate::options::Autostart;

    #[test]
    fn the_service_starts_in_its_profile_directory_not_the_callers() {
        let home = tempfile::tempdir().expect("a temporary home");
        let profile = Profile::resolve_with(
            Some(home.path().to_path_buf()),
            Some("default".to_owned()),
            |_| None,
        )
        .expect("the profile resolves");
        let autostart = Autostart {
            exe: PathBuf::from("massorca"),
            args_extra: Vec::new(),
        };

        let command = service_command(&profile, &autostart);

        assert_eq!(command.get_current_dir(), Some(profile.dir()));
    }
}
