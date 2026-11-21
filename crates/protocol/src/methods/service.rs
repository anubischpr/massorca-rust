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
}
