//! The `massorca` command: runs and controls the service of a profile, and reads and changes its settings.
//!
//! The binary is a thin shell over [`run`], so parsing, output and exit codes stay testable.

pub mod agent;
pub mod agent_accounts;
pub mod agent_context;
pub mod agent_credentials;
pub mod agent_hook;
pub mod agent_text;
pub mod app;
pub mod args;
pub mod cli_install;
pub mod config;
pub mod connect;
pub mod cwd_path;
pub mod device;
pub mod error;
pub mod extension;
pub mod extension_text;
pub mod git;
pub mod host;
pub mod logging;
pub mod open;
pub mod output;
pub mod pair;
pub mod profile_secure;
pub mod project;
pub mod project_icon;
pub mod secret;
pub mod secret_input;
pub mod server_status;
pub mod service;
pub mod skills;
pub mod stream_reconnect;
pub mod target;
pub mod task;
pub mod team;
pub mod terminal;
pub mod terminal_attach;
pub mod terminal_wait;
pub mod time_text;
pub mod workspace;
pub mod workspace_text;

pub use app::run;
