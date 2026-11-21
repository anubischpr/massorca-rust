//! Starts `massorca service run` detached from this process (ADR 0003), so the service outlives the client that started it.

use std::process::{Command, Stdio};

use massorca_profile::Profile;

use crate::error::ClientError;
use crate::options::Autostart;

pub(crate) fn service_run(profile: &Profile, autostart: &Autostart) -> Result<(), ClientError> {
    let mut command = Command::new(&autostart.exe);
    command
        .args(["service", "run", "--profile", profile.name()])
        .args(&autostart.args_extra)
        .env("MASSORCA_HOME", profile.home())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut command);
    keep_handles_private()?;
    command
        .spawn()
        .map(drop)
        .map_err(|error| ClientError::StartFailed(format!("{}: {error}", autostart.exe.display())))
}

/// Stops the detached service from inheriting this process's standard pipes, so a caller reading them to end-of-file is not held open.
#[cfg(windows)]
fn keep_handles_private() -> Result<(), ClientError> {
    crate::windows_handle_inheritance::clear_standard_handle_inheritance().map_err(|error| {
        ClientError::StartFailed(format!(
            "could not stop the service inheriting handles: {error}"
        ))
    })
}

#[cfg(unix)]
fn keep_handles_private() -> Result<(), ClientError> {
    Ok(())
}

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
