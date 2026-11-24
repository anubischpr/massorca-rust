//! Command methods: the command table the service keeps, which `massorca agent-context` and palettes read.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Params: none. Result: [`CommandTable`].
pub const DESCRIBE: &str = "commands.describe";

/// Result of `commands.describe`: every command the service runs, ordered by id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandTable {
    pub commands: Vec<CommandEntry>,
}

/// One command: its CLI path, what it does, and the request it issues first with what that request needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandEntry {
    /// The CLI path, dotted: `git.status` is `massorca git status`.
    pub id: String,
    /// One line that says what the command does.
    pub title: String,
    /// The first request the command issues; a composed command is not one call.
    pub method: String,
    /// The capability the method belongs to; null when the service registers no such method.
    pub capability: Option<String>,
    /// `read`, `workspace`, `host-admin` or `device-admin`; null when the service registers no such method.
    pub permission: Option<String>,
    /// JSON Schema of the params: `required` and `properties` name each parameter and its type.
    pub args_schema: Value,
}
