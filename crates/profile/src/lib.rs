//! Per-profile layout and the service endpoint name (ADR 0004).

mod endpoint;
mod error;
mod profile;
mod root_access;
#[cfg(unix)]
mod unix_user;
#[cfg(windows)]
mod windows_endpoint_access;
#[cfg(windows)]
mod windows_profile_root;
#[cfg(windows)]
mod windows_sddl;
#[cfg(windows)]
mod windows_sid;

pub use endpoint::local_socket_name;
#[cfg(unix)]
pub use endpoint::restrict_socket;
pub use error::ProfileError;
pub use profile::{Profile, ensure_dir};
pub use root_access::{MAX_LISTED_ENTRIES, RootAccess, RootAccessError, secure_root};
#[cfg(unix)]
pub use unix_user::current_uid;
#[cfg(windows)]
pub use windows_endpoint_access::{
    owner_only_sddl, owner_only_security_descriptor, process_belongs_to_current_user,
};
