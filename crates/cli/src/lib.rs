//! The `massorca` command: runs and controls the service of a profile, and reads and changes its settings.
//!
//! The binary is a thin shell over [`run`], so parsing, output and exit codes stay testable.

pub mod agent;
pub mod agent_accounts;
pub mod agent_credentials;
pub mod agent_hook;
pub mod agent_text;
pub mod app;
pub mod args;
pub mod config;
pub mod connect;
pub mod cwd_path;
pub mod error;
pub mod ext;
pub mod git;
pub mod logging;
pub mod output;
pub mod project;
pub mod service;
pub mod terminal;
pub mod terminal_attach;
pub mod workspace;
pub mod workspace_text;

pub use app::run;
