//! SQLite persistence for MassOrca's durable state (ADR 0005): projects, workspaces, settings, agent sessions, paired
//! devices and vault rows, with versioned migrations and a backup taken before each migration.

mod agent_accounts;
mod agent_sessions;
mod backup;
mod bots;
mod devices;
mod error;
mod migrations;
mod projects;
mod secret_key_check;
mod secrets;
mod settings;
mod store;
mod tasks;
mod team_memory;
mod team_projects;
mod teams;
mod workspaces;

use std::time::{SystemTime, UNIX_EPOCH};

pub use agent_accounts::AgentAccountRow;
pub use agent_sessions::{AgentSessionRow, RunExit, RunExitReason, StoredActivity};
pub use bots::BotRow;
pub use devices::DeviceRow;
pub use error::StoreError;
pub use migrations::{MIGRATIONS, Migration};
pub use projects::{ProjectKind, ProjectRow};
pub use secret_key_check::SecretKeyCheck;
pub use secrets::{SecretLinkRow, SecretMeta, SecretRequestRow, SecretRow};
pub use store::Store;
pub use tasks::{TaskOrigin, TaskOutcome, TaskRow};
pub use team_memory::TeamMemoryRow;
pub use team_projects::TeamProjectRow;
pub use teams::TeamRow;
pub use workspaces::{WorkspaceKind, WorkspaceRow, WorkspaceState};

/// A new UUID v7 string; ids sort by creation time.
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}
