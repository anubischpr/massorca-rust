//! SQLite persistence for MassOrca's durable state (ADR 0005): projects, workspaces and settings,
//! with versioned migrations and a backup taken before each migration.

mod backup;
mod error;
mod migrations;
mod projects;
mod store;
mod workspaces;

use std::time::{SystemTime, UNIX_EPOCH};

pub use error::StoreError;
pub use migrations::MIGRATIONS;
pub use projects::{ProjectKind, ProjectRow};
pub use store::Store;
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
