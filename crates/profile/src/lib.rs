//! Per-profile layout and the service endpoint name (ADR 0004).

mod endpoint;
mod error;
mod profile;

pub use endpoint::local_socket_name;
pub use error::ProfileError;
pub use profile::{Profile, ensure_dir};
