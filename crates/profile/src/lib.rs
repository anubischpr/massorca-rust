//! Per-profile layout and the service endpoint name (ADR 0004).

mod endpoint;
mod error;
mod profile;
mod root_access;
#[cfg(windows)]
mod scan_cache;
#[cfg(unix)]
mod unix_profile_root;
#[cfg(unix)]
mod unix_user;
#[cfg(windows)]
mod windows_dir_access;
#[cfg(windows)]
mod windows_endpoint_access;
#[cfg(windows)]
mod windows_profile_root;
#[cfg(windows)]
mod windows_profile_secure;
#[cfg(windows)]
mod windows_sddl;
#[cfg(windows)]
mod windows_trust;

pub use endpoint::local_socket_name;
#[cfg(unix)]
pub use endpoint::restrict_socket;
pub use error::ProfileError;
pub use profile::{Profile, ensure_dir};
pub use root_access::{
    FolderPlan, MAX_LISTED_ENTRIES, RootAccess, RootAccessError, apply_secure, create_home,
    create_private_dir, create_root, plan_secure, secure_root,
};
#[cfg(unix)]
pub use unix_user::current_uid;
#[cfg(windows)]
pub use windows_dir_access::{
    PinnedDir, Sid, create_dir_owner_only, dacl_grantees, dacl_sddl, dir_owner_is_acceptable,
    has_owner_only_dacl, owner_is_acceptable, owner_matches, owner_only_dir_sddl,
    restrict_dir_to_owner,
};
#[cfg(windows)]
pub use windows_endpoint_access::{
    owner_only_sddl, owner_only_security_descriptor, process_belongs_to_current_user,
};
