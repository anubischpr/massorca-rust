//! The `massorca` command: runs and controls the service of a profile, and reads and changes its settings.
//!
//! The binary is a thin shell over [`run`], so parsing, output and exit codes stay testable.

pub mod app;
pub mod args;
pub mod error;
pub mod ext;
pub mod output;

pub use app::run;
