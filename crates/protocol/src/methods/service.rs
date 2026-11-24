//! Service methods: identity and lifecycle of the local service.

use serde::{Deserialize, Serialize};

/// Params: none. Result: [`ServiceInfo`].
pub const INFO: &str = "service.info";
/// Params: none. Result: `null`.
pub const SHUTDOWN: &str = "service.shutdown";

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
