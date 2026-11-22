//! The MassOrca extension package format: manifest, contribution kinds, permissions, risk, uninstall effects and the
//! archive check.
//!
//! Pure data and checks. The crate reads only the bytes its callers hand in and writes nothing to disk.

pub mod archive;
pub mod display_chars;
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
