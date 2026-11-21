//! The exclusive per-profile lock that makes a service the only one for its profile (ADR 0003).

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::time::Duration;

use interprocess::local_socket::Name;
use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::traits::tokio::Stream as _;
use massorca_profile::{Profile, local_socket_name};
use tokio::time::{Instant, sleep};

use crate::error::ServiceError;

/// How long a starting service waits for a lock that nothing answers for. A predecessor that has stopped listening may
/// still be closing its connections, which takes up to the five-second shutdown grace.
const LOCK_WAIT: Duration = Duration::from_secs(10);
const LOCK_POLL: Duration = Duration::from_millis(50);

/// Takes `<profile>/service.lock`. The operating system releases it when the returned file closes, even after a crash.
/// A lock held while a service answers on the endpoint is refused at once. A lock held while nothing answers is waited
/// for, so a successor can start while its predecessor is still draining.
pub(crate) async fn acquire(profile: &Profile) -> Result<File, ServiceError> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(profile.lock_path())?;
    let endpoint = local_socket_name(profile)?;
    let deadline = Instant::now() + LOCK_WAIT;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) => {
                if answers(endpoint.clone()).await || Instant::now() >= deadline {
                    return Err(ServiceError::AlreadyRunning);
                }
                sleep(LOCK_POLL).await;
            }
            Err(TryLockError::Error(error)) => return Err(error.into()),
        }
    }
}

/// Whether a service accepts connections on `endpoint` right now.
async fn answers(endpoint: Name<'static>) -> bool {
    Stream::connect(endpoint).await.is_ok()
}

/// Removes a socket file left behind by a service that died without cleaning up. Call only while holding the lock.
#[cfg(unix)]
pub(crate) fn remove_stale_socket(profile: &Profile) -> io::Result<()> {
    match std::fs::remove_file(profile.socket_path()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Windows named pipes go away with their server, so there is no file to remove.
#[cfg(not(unix))]
pub(crate) fn remove_stale_socket(_profile: &Profile) -> io::Result<()> {
    Ok(())
}
