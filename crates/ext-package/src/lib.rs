//! The MassOrca extension package format: manifest, contribution kinds and permissions.
//!
//! Pure data and checks. The crate reads only the bytes its callers hand in and writes nothing to disk.

mod error;
pub mod ids;
pub mod package_path;

pub use error::{Error, Problem, codes};
