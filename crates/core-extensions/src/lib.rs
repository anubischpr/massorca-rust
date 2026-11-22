//! Installed extensions per profile (decision 0062): the lockfile, install sources, the built-in catalogue, the review
//! and consent, the vendor CLI installer, enable, disable and remove.
//!
//! No protocol I/O and no dependency on `core-agents`: program resolution and process running come in as traits.

pub mod catalogue;
mod error;
pub mod host;
pub mod layout;
pub mod lockfile;
pub mod review;
pub mod service;
pub mod source;
pub mod views;

pub use error::{Error, HookError, codes};
pub use layout::Layout;
pub use lockfile::Lockfile;
pub use service::Extensions;
