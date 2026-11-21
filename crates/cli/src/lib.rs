//! The `massorca` command: runs and controls the service of a profile, and reads and changes its settings.
//!
//! The binary is a thin shell over [`run`], so parsing, output and exit codes stay testable.

pub mod app;
pub mod args;
pub mod config;
pub mod connect;
pub mod error;
pub mod ext;
pub mod logging;
pub mod output;
pub mod service;

pub use app::run;
