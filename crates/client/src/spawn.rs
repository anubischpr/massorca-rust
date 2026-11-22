//! Starts `massorca service run` detached from this process (ADR 0003), so the service outlives the client that started it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

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
    create_unique_dir(&std::env::temp_dir(), status_dir_name)
}

/// Starts this process has made; with random bytes it names each status folder, since two starts from one process can
/// read the same clock value.
static STARTS: AtomicU64 = AtomicU64::new(0);

/// How many names are tried before a start gives up on its status folder.
const STATUS_DIR_ATTEMPTS: usize = 8;

fn status_dir_name() -> std::io::Result<String> {
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).map_err(|_| std::io::Error::other("no random source"))?;
    Ok(format!(
        "massorca-start-{}-{}-{:016x}",
        std::process::id(),
        STARTS.fetch_add(1, Ordering::Relaxed),
        u64::from_ne_bytes(random)
    ))
}

/// Creates a folder under `parent` that did not exist, so this start alone owns it; a taken name is retried with the
/// next one `name` gives.
fn create_unique_dir(
    parent: &Path,
    mut name: impl FnMut() -> std::io::Result<String>,
) -> std::io::Result<PathBuf> {
    for _ in 0..STATUS_DIR_ATTEMPTS {
        let dir = parent.join(name()?);
        if massorca_profile::create_private_dir(&dir)? {
            return Ok(dir);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!("no free name in {}", parent.display()),
    ))
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

/// The service starts in its profile directory, so it must exist. Missing folders are created private to their owner in
/// one step, never with inherited access the service would only repair later (ADR 0073); the service checks the rest.
fn create_profile_dir(profile: &Profile) -> Result<(), ClientError> {
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

    /// Starts from one process at the same instant, as two clients joined in one task are, each get a folder of their
    /// own, and releasing one leaves the others in place.
    #[test]
    fn concurrent_starts_from_one_process_each_get_their_own_status_folder() {
        const THREADS: usize = 16;
        const ROUNDS: usize = 64;
        let barrier = std::sync::Barrier::new(THREADS);
        let dirs: Vec<PathBuf> = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..THREADS)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        (0..ROUNDS)
                            .map(|_| super::status_dir().expect("a status folder of its own"))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            workers
                .into_iter()
                .flat_map(|worker| worker.join().expect("the worker finishes"))
                .collect()
        });

        let unique: std::collections::BTreeSet<&PathBuf> = dirs.iter().collect();
        assert_eq!(unique.len(), THREADS * ROUNDS);
        std::fs::remove_dir_all(&dirs[0]).expect("one start releases its folder");
        assert!(dirs[1..].iter().all(|dir| dir.is_dir()));
        for dir in &dirs[1..] {
            std::fs::remove_dir_all(dir).expect("removed");
        }
    }

    #[test]
    fn a_status_folder_name_that_is_taken_is_retried_and_the_existing_folder_is_left_alone() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let taken = temp.path().join("taken");
        std::fs::create_dir(&taken).expect("a folder another start owns");
        let mut names = ["taken", "free"]
            .into_iter()
            .map(|name| Ok(name.to_owned()));

        let dir = super::create_unique_dir(temp.path(), || names.next().expect("a name"))
            .expect("the next name");

        assert_eq!(dir, temp.path().join("free"));
        assert!(taken.is_dir());
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
}
