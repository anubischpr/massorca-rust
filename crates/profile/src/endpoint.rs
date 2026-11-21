//! The service endpoint: a named pipe on Windows, a socket file on Unix.

use std::io;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

use interprocess::local_socket::Name;
#[cfg(unix)]
use interprocess::local_socket::{GenericFilePath, ToFsName};
#[cfg(windows)]
use interprocess::local_socket::{GenericNamespaced, ToNsName};

use crate::profile::Profile;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// `sun_path` is 104 bytes on the strictest platforms; the path must leave room for its NUL.
#[cfg(unix)]
const MAX_SOCKET_PATH_BYTES: usize = 104;

/// FNV-1a, 64-bit.
pub(crate) fn fnv1a_64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV_OFFSET_BASIS, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    })
}

/// The name the service listens on and clients connect to.
#[cfg(windows)]
pub fn local_socket_name(profile: &Profile) -> io::Result<Name<'static>> {
    profile.pipe_name().to_ns_name::<GenericNamespaced>()
}

/// The name the service listens on and clients connect to. Rejects socket paths that do not fit `sun_path`.
#[cfg(unix)]
pub fn local_socket_name(profile: &Profile) -> io::Result<Name<'static>> {
    let path = profile.socket_path();
    let len = path.as_os_str().as_bytes().len();
    if len >= MAX_SOCKET_PATH_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "socket path {} is {len} bytes, but Unix sockets must be shorter than {MAX_SOCKET_PATH_BYTES} bytes; use a shorter home directory (MASSORCA_HOME)",
                path.display()
            ),
        ));
    }
    path.into_os_string().to_fs_name::<GenericFilePath>()
}

/// Restricts the socket file to its owner (0600), after the listener binds it. The directory is already private, so no other user can reach the file in that moment.
#[cfg(unix)]
pub fn restrict_socket(profile: &Profile) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
        profile.socket_path(),
        std::fs::Permissions::from_mode(0o600),
    )
}

#[cfg(test)]
mod tests {
    use super::fnv1a_64;

    #[test]
    fn matches_published_fnv1a_vectors() {
        assert_eq!(fnv1a_64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a_64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a_64(b"foobar"), 0x8594_4171_f739_67e8);
    }
}
