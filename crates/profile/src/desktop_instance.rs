//! The single-instance rule of the desktop (PLAT-07). The first desktop of a profile holds `desktop/instance.lock` and
//! answers activate requests on a local endpoint; a later launch sends one and leaves.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, BufRead, BufReader, Read, Write};
#[cfg(unix)]
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use interprocess::local_socket::traits::{ListenerExt as _, Stream as _};
use interprocess::local_socket::{Listener, ListenerOptions, Stream};
#[cfg(windows)]
use interprocess::os::windows::local_socket::ListenerOptionsExt;

use crate::desktop_dir::open_desktop_dir;
use crate::endpoint::desktop_socket_name;
#[cfg(unix)]
use crate::endpoint::restrict_socket_file;
use crate::profile::Profile;
#[cfg(windows)]
use crate::windows_endpoint_access::owner_only_security_descriptor;

/// The request a later launch sends. Nothing else is acted on.
const ACTIVATE: &str = "activate";
/// The answer a running desktop sends once it has acted on the request.
const ACKNOWLEDGED: &str = "ok";
/// The longest request read; a longer line is never acted on.
const MAX_REQUEST_BYTES: u64 = 64;
/// How long a launch that finds the lock held keeps trying the endpoint, since its owner may still be binding it.
const HANDOFF_WAIT: Duration = Duration::from_secs(3);
const HANDOFF_PAUSE: Duration = Duration::from_millis(50);

/// What a desktop launch got for its profile.
#[derive(Debug)]
pub enum DesktopClaim {
    /// This launch is the profile's desktop; call `serve` on the owner to answer the launches after it.
    Owner(DesktopOwner),
    /// The running desktop acknowledged the request, so this launch should end.
    HandedOff,
}

/// The lock of the profile's desktop and the endpoint it answers on.
#[derive(Debug)]
pub struct DesktopOwner {
    lock: File,
    listener: Listener,
}

/// Takes the profile's desktop lock, or hands an activate request to the desktop that holds it.
pub fn claim_desktop(profile: &Profile) -> io::Result<DesktopClaim> {
    open_desktop_dir(profile).map_err(io::Error::other)?;
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(profile.desktop_lock_path())?;
    match lock.try_lock() {
        Ok(()) => {
            let listener = bind(profile)?;
            Ok(DesktopClaim::Owner(DesktopOwner { lock, listener }))
        }
        Err(TryLockError::WouldBlock) => {
            hand_off(profile, HANDOFF_WAIT)?;
            Ok(DesktopClaim::HandedOff)
        }
        Err(TryLockError::Error(error)) => Err(error),
    }
}

impl DesktopOwner {
    /// Answers later launches on a background thread for as long as the process runs. Each acknowledged activate
    /// request calls `on_activate` before its launch is answered.
    pub fn serve(self, on_activate: impl Fn() + Send + Sync + 'static) -> io::Result<()> {
        let Self { lock, listener } = self;
        let on_activate = Arc::new(on_activate);
        thread::Builder::new()
            .name("desktop-instance".to_owned())
            .spawn(move || {
                // Held by this thread, so the profile stays claimed until the process ends.
                let _lock = lock;
                for connection in listener.incoming().flatten() {
                    let on_activate = Arc::clone(&on_activate);
                    // One thread per request, so a client that stalls cannot hold up the others.
                    let _ = thread::Builder::new()
                        .name("desktop-instance-request".to_owned())
                        .spawn(move || answer(connection, &*on_activate));
                }
            })?;
        Ok(())
    }
}

/// Binds the endpoint. Only the lock holder gets here, so a socket file a dead desktop left behind is replaced.
fn bind(profile: &Profile) -> io::Result<Listener> {
    let name = desktop_socket_name(profile)?;
    #[cfg(unix)]
    remove_stale_socket(&profile.desktop_socket_path())?;
    #[cfg(windows)]
    let listener = ListenerOptions::new()
        .name(name)
        .security_descriptor(owner_only_security_descriptor()?)
        .create_sync()?;
    #[cfg(unix)]
    let listener = ListenerOptions::new().name(name).create_sync()?;
    #[cfg(unix)]
    restrict_socket_file(&profile.desktop_socket_path())?;
    Ok(listener)
}

/// Deletes a socket file left behind, which only the lock holder may do.
#[cfg(unix)]
fn remove_stale_socket(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Sends the activate request and waits for the acknowledgement, retrying while the endpoint is not reachable yet.
fn hand_off(profile: &Profile, wait: Duration) -> io::Result<()> {
    let name = desktop_socket_name(profile)?;
    let deadline = Instant::now() + wait;
    loop {
        match Stream::connect(name.clone()) {
            Ok(stream) => return exchange(stream),
            Err(_) if Instant::now() < deadline => thread::sleep(HANDOFF_PAUSE),
            Err(error) => return Err(error),
        }
    }
}

/// Writes the activate request and reads the reply; anything but the acknowledgement is an error.
fn exchange(mut stream: Stream) -> io::Result<()> {
    stream.write_all(format!("{ACTIVATE}\n").as_bytes())?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
    if reply.trim_end() == ACKNOWLEDGED {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "the running desktop did not acknowledge the request",
    ))
}

/// Reads one request and acknowledges it when it is the activate request.
fn answer<F: Fn()>(mut stream: Stream, on_activate: &F) -> io::Result<()> {
    let mut request = String::new();
    BufReader::new((&mut stream).take(MAX_REQUEST_BYTES)).read_line(&mut request)?;
    if request.trim_end() != ACTIVATE {
        return Ok(());
    }
    on_activate();
    stream.write_all(format!("{ACKNOWLEDGED}\n").as_bytes())
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    /// A short prefix keeps the Unix socket path under its length limit on every platform.
    fn temp() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("mdi")
            .tempdir()
            .expect("a temporary folder")
    }

    fn profile(home: &Path, name: &str) -> Profile {
        Profile::resolve_with(Some(home.join("h")), Some(name.to_owned()), |_| None)
            .expect("a profile")
    }

    fn owner(profile: &Profile) -> DesktopOwner {
        match claim_desktop(profile).expect("claimed") {
            DesktopClaim::Owner(owner) => owner,
            DesktopClaim::HandedOff => panic!("nothing else holds the profile"),
        }
    }

    #[test]
    fn a_later_launch_hands_its_request_to_the_owner_and_the_handler_runs_once() {
        let temp = temp();
        let profile = profile(temp.path(), "work");
        let activations = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&activations);
        owner(&profile)
            .serve(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            })
            .expect("serving");

        let second = claim_desktop(&profile).expect("handed off");

        assert!(matches!(second, DesktopClaim::HandedOff));
        assert_eq!(activations.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_request_other_than_activate_is_not_acted_on_or_acknowledged() {
        let temp = temp();
        let profile = profile(temp.path(), "work");
        let activations = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&activations);
        owner(&profile)
            .serve(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            })
            .expect("serving");
        let name = desktop_socket_name(&profile).expect("a name");

        let mut stream = Stream::connect(name).expect("connected");
        stream.write_all(b"quit\n").expect("sent");
        let mut reply = String::new();
        BufReader::new(stream)
            .read_line(&mut reply)
            .expect("the connection closes");

        assert!(reply.is_empty());
        assert_eq!(activations.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn the_lock_is_released_when_its_owner_is_dropped() {
        let temp = temp();
        let profile = profile(temp.path(), "work");
        drop(owner(&profile));

        assert!(matches!(
            claim_desktop(&profile).expect("claimed again"),
            DesktopClaim::Owner(_)
        ));
    }

    #[test]
    fn the_endpoints_are_named_per_profile_and_apart_from_the_service() {
        let temp = temp();
        let work = profile(temp.path(), "work");
        let other = profile(temp.path(), "other");

        assert!(
            work.desktop_pipe_name()
                .starts_with("massorca-desktop-work-")
        );
        assert_ne!(work.desktop_pipe_name(), other.desktop_pipe_name());
        assert_ne!(work.desktop_pipe_name(), work.pipe_name());
        assert_eq!(
            work.desktop_lock_path(),
            work.dir().join("desktop").join("instance.lock")
        );
        assert!(desktop_socket_name(&work).is_ok());
    }

    #[test]
    fn a_launch_that_finds_nothing_answering_gives_up_after_the_wait() {
        let temp = temp();
        let profile = profile(temp.path(), "work");
        let started = Instant::now();

        let gave_up = hand_off(&profile, Duration::from_millis(100));

        assert!(gave_up.is_err(), "no owner answers");
        assert!(started.elapsed() >= Duration::from_millis(100));
    }

    #[cfg(unix)]
    #[test]
    fn a_socket_file_a_dead_desktop_left_behind_is_replaced() {
        let temp = temp();
        let profile = profile(temp.path(), "work");
        open_desktop_dir(&profile).expect("the desktop folder");
        fs::write(profile.desktop_socket_path(), b"stale").expect("a leftover file");

        assert!(matches!(
            claim_desktop(&profile).expect("claimed"),
            DesktopClaim::Owner(_)
        ));
    }
}
