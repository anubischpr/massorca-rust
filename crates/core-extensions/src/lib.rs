//! Installed extensions per profile (decision 0062): the lockfile, install sources, the built-in catalogue, the review
//! and consent, the vendor CLI installer, enable, disable and remove.
//!
//! No protocol I/O and no dependency on `core-agents`: program resolution and process running come in as traits.

pub mod agents;
pub mod catalogue;
mod error;
pub mod hooks;
pub mod host;
pub mod item;
pub mod layout;
pub mod lockfile;
pub mod removal;
pub mod review;
pub mod service;
pub mod source;
mod sweep;
pub mod views;

pub use agents::{
    CatalogueAgentKind, InstalledAgentKind, PackageAgentKind, catalogue_agent_kinds,
    installed_agent_kinds, package_agent_kinds,
};
pub use error::{Error, HookError, codes};
pub use layout::Layout;
pub use lockfile::Lockfile;
pub use service::Extensions;
