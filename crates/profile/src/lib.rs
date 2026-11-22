//! Per-profile layout and the service endpoint name (ADR 0004).

mod endpoint;
mod error;
mod profile;
#[cfg(unix)]
mod unix_user;
#[cfg(windows)]
mod windows_dir_access;
#[cfg(windows)]
mod windows_endpoint_access;

pub use endpoint::local_socket_name;
#[cfg(unix)]
pub use endpoint::restrict_socket;
pub use error::ProfileError;
pub use profile::{Profile, ensure_dir};
#[cfg(unix)]
pub use unix_user::current_uid;
#[cfg(windows)]
pub use windows_dir_access::{
    PinnedDir, create_dir_owner_only, dacl_sddl, has_owner_only_dacl, owner_only_dir_sddl,
    restrict_dir_to_owner,
};
#[cfg(windows)]
pub use windows_endpoint_access::{
    owner_only_sddl, owner_only_security_descriptor, process_belongs_to_current_user,
};
