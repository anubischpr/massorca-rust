//! Per-profile layout and the service endpoint name (ADR 0004).

mod endpoint;
mod error;
mod profile;
#[cfg(unix)]
mod unix_user;
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
pub use windows_endpoint_access::{
    dacl_sddl, is_owner_only, owner_only_sddl, owner_only_security_descriptor,
    process_belongs_to_current_user, restrict_to_owner, set_dacl_sddl,
};
