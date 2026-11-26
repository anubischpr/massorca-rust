//! Service methods: identity and lifecycle of the local service.

use serde::{Deserialize, Serialize};

/// Params: none. Result: [`ServiceInfo`].
pub const INFO: &str = "service.info";
/// Params: [`ToolsParams`]. Result: [`ToolList`]. Capability `service-tools/1`.
pub const TOOLS: &str = "service.tools";
/// Params: none. Result: `null`.
pub const SHUTDOWN: &str = "service.shutdown";

/// Params of `service.tools`. An absent `refresh` answers from the last detection of this service run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolsParams {
    #[serde(default)]
    pub refresh: bool,
}

/// Result of `service.tools`: the developer tools the service's host has, one entry per tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolList {
    pub tools: Vec<ToolInfo>,
}

/// One tool on the service's host. `id` is open: a client keeps an id it does not know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInfo {
    pub id: String,
    pub kind: ToolKind,
    pub state: ToolState,
    /// The version the tool printed, when it was asked and printed one; `null` otherwise.
    pub version: Option<String>,
    /// The file the service would run, when the tool was found; `null` otherwise.
    pub path: Option<String>,
}

/// What a tool is for. A kind this client does not know decodes as `other`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    /// A version control program: git.
    Vcs,
    /// A hosting service's command line: gh, glab.
    Forge,
    /// A command shell a terminal can run.
    Shell,
    #[serde(other)]
    Other,
}

/// Whether a tool is usable. A state this client does not know decodes as `other`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolState {
    Present,
    Missing,
    /// Found, but older than the version the service requires.
    TooOld,
    /// Found, but the service could not read a version it trusts (a hang, a failure, no version printed).
    Unknown,
    #[serde(other)]
    Other,
}

/// Result of `service.info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub version: String,
    pub protocol: u32,
    pub capabilities: Vec<String>,
    pub profile: String,
    pub pid: u32,
    pub started_at_ms: i64,
    /// The host's git, probed when the request is answered; absent from a service that predates it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<GitStatus>,
}

/// The host's git: its version, or why the service cannot use it (ADR 0014).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum GitStatus {
    Present { version: String },
    Missing,
    TooOld { version: String, required: String },
    Unknown { reason: String },
}
