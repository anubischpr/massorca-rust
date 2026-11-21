//! SQLite persistence for MassOrca's durable state (ADR 0005): projects, workspaces and settings,
//! with versioned migrations and a backup taken before each migration.

mod backup;
mod error;
mod migrations;
mod store;

pub use error::StoreError;
pub use migrations::MIGRATIONS;
pub use store::Store;
