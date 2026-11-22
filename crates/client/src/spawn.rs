//! Starts `massorca service run` detached from this process (ADR 0003), so the service outlives the client that started it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use massorca_profile::Profile;
use massorca_protocol::ErrorBody;

use crate::error::ClientError;
use crate::options::Autostart;

/// The hidden `service run` option that names the file a starting service writes its refusal to, as one `ErrorBody`.
/// An option, not an environment variable, so it never reaches the terminals the service starts.
const START_STATUS_OPTION: &str = "--start-status";

/// A service this client started: its process, and the private folder its start status goes to.
#[derive(Debug)]
pub(crate) struct Starting {
    child: Child,
    status_dir: PathBuf,
}

impl Starting {
    /// The refusal, once the service has exited before accepting connections; `None` while it still runs. The status
    /// file is read only after the exit status says the start failed.
    pub(crate) fn refusal(&mut self) -> Option<ClientError> {
        let status = self.child.try_wait().ok().flatten()?;
        Some(read_refusal(&self.status_dir.join(STATUS_FILE), status))
    }

    /// Lets the service run on without this client, and removes the status folder.
    pub(crate) fn release(self) {
        let _ = fs::remove_dir_all(&self.status_dir);
        reap_when_it_exits(self.child);
    }
}

const STATUS_FILE: &str = "status.json";

/// The `ErrorBody` in a status file, if it is no larger than `MAX_STATUS_BYTES`.
fn read_status(path: &Path) -> Option<ErrorBody> {
    use std::io::Read;

    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(MAX_STATUS_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_STATUS_BYTES {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn service_run(
    profile: &Profile,
    autostart: &Autostart,
) -> Result<Starting, ClientError> {
    create_profile_dir(profile)?;
    let status_dir = status_dir().map_err(|error| {
        ClientError::StartFailed(format!("cannot create the start status folder: {error}"))
    })?;
    let mut command = service_command(profile, autostart);
    command
        .arg(START_STATUS_OPTION)
        .arg(status_dir.join(STATUS_FILE));
    detach(&mut command);
    keep_handles_private();
    let child = command.spawn().map_err(|error| {
        let _ = fs::remove_dir_all(&status_dir);
        ClientError::StartFailed(format!("{}: {error}", autostart.exe.display()))
    })?;
    Ok(Starting { child, status_dir })
}

/// A new folder only this user can reach, outside the profile: a refused profile is never written to.
fn status_dir() -> std::io::Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("massorca-start-{}-{nanos}", std::process::id()));
    if massorca_profile::create_private_dir(&dir)? {
        Ok(dir)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{} already exists", dir.display()),
        ))
    }
}

/// A status file larger than this is ignored.
const MAX_STATUS_BYTES: u64 = 64 * 1024;

/// The error a service that exited with a failure left in its status file, or a plain start failure when it exited
/// otherwise or left none. Read with a size bound.
fn read_refusal(path: &Path, status: ExitStatus) -> ClientError {
    let body = if status.success() {
        None
    } else {
        read_status(path)
    };
    match body {
        Some(body) => ClientError::StartRefused(Box::new(body)),
        None => ClientError::StartFailed(format!(
            "the service exited ({status}) before accepting connections"
        )),
    }
}

/// The service starts in its profile directory, so it must exist. The location is checked first, without creating
/// anything, so a link at the home or `profiles` cannot redirect where folders are made; then missing folders are
/// created private to their owner in one step (ADR 0073). The service checks the rest.
fn create_profile_dir(profile: &Profile) -> Result<(), ClientError> {
    massorca_profile::check_location(profile)?;
    massorca_profile::create_root(profile)
        .map(drop)
        .map_err(|error| ClientError::StartFailed(format!("{}: {error}", profile.dir().display())))
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

    use super::{create_profile_dir, service_command};
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

    #[test]
    fn the_autostart_path_creates_the_profile_directory_private_so_nothing_needs_repair() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = Profile::resolve_with(
            Some(temp.path().join("home")),
            Some("default".to_owned()),
            |_| None,
        )
        .expect("the profile resolves");

        create_profile_dir(&profile).expect("the profile directory");

        assert_eq!(
            massorca_profile::secure_root(&profile).expect("checked"),
            massorca_profile::RootAccess::Private
        );
    }

    fn exit_status(code: u32) -> std::process::ExitStatus {
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(code)
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            std::process::ExitStatus::from_raw((code as i32) << 8)
        }
    }

    #[test]
    fn a_status_file_is_trusted_only_after_a_failed_exit_and_only_when_small() {
        use super::{MAX_STATUS_BYTES, read_refusal};
        use crate::ClientError;

        let temp = tempfile::tempdir().expect("a temporary folder");
        let status = temp.path().join("status.json");
        std::fs::write(
            &status,
            r#"{"code":"profile.unsafe_location","message":"unsafe"}"#,
        )
        .expect("a status");

        assert!(matches!(
            read_refusal(&status, exit_status(1)),
            ClientError::StartRefused(body) if body.code == "profile.unsafe_location"
        ));
        assert!(
            matches!(
                read_refusal(&status, exit_status(0)),
                ClientError::StartFailed(_)
            ),
            "a service that exited cleanly left no refusal"
        );

        let padding = " ".repeat(MAX_STATUS_BYTES as usize);
        std::fs::write(&status, format!(r#"{{"code":"x","message":"y"}}{padding}"#))
            .expect("an oversized status");
        assert!(matches!(
            read_refusal(&status, exit_status(1)),
            ClientError::StartFailed(_)
        ));
    }

    #[test]
    fn the_autostart_path_creates_nothing_through_a_link_at_profiles() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let home = temp.path().join("home");
        massorca_profile::create_home(&home).expect("a private home");
        let elsewhere = temp.path().join("elsewhere");
        std::fs::create_dir(&elsewhere).expect("a folder");
        let profiles = home.join("profiles");
        #[cfg(windows)]
        {
            let made = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&profiles)
                .arg(&elsewhere)
                .output()
                .expect("mklink runs");
            assert!(
                made.status.success(),
                "{}",
                String::from_utf8_lossy(&made.stderr)
            );
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&elsewhere, &profiles).expect("a symlink");
        let profile = Profile::resolve_with(Some(home), Some("default".to_owned()), |_| None)
            .expect("the profile resolves");

        match create_profile_dir(&profile) {
            Err(crate::ClientError::ProfileRefused(body)) => {
                assert_eq!(body.code, "profile.unsafe_location", "{body:?}");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        let created: Vec<_> = std::fs::read_dir(&elsewhere).expect("listed").collect();
        assert!(created.is_empty(), "{created:?}");
    }
}
