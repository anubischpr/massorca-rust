//! The exclusive per-profile lock that makes a service the only one for its profile (ADR 0003).

use std::fs::{File, OpenOptions, TryLockError};
use std::io;

use massorca_profile::Profile;

use crate::error::ServiceError;

/// Takes `<profile>/service.lock`. The operating system releases it when the returned file closes, even after a crash.
pub(crate) fn acquire(profile: &Profile) -> Result<File, ServiceError> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(profile.lock_path())?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(ServiceError::AlreadyRunning),
        Err(TryLockError::Error(error)) => Err(error.into()),
    }
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
