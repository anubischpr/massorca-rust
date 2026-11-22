//! The MassOrca extension package format: manifest, contribution kinds and permissions.
//!
//! Pure data and checks. The crate reads only the bytes its callers hand in and writes nothing to disk.

mod error;
pub mod ids;
pub mod manifest;
pub mod package_path;
pub mod permission;
pub mod risk;
pub mod uninstall;
pub mod update;

pub use error::{Error, Problem, codes};
pub use manifest::{Manifest, load, parse, validate};
