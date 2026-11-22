//! Secret vault methods (capability `secrets/1`, decision 0071). No result ever carries a secret's value.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Params: none. Result: [`SecretStatus`].
pub const STATUS: &str = "secret.status";
/// Params: [`SecretListParams`]. Result: [`SecretList`].
pub const LIST: &str = "secret.list";
/// Params: [`SecretSet`]. Result: [`SecretEntry`].
pub const SET: &str = "secret.set";
/// Params: [`SecretLink`]. Result: [`SecretEntry`].
pub const LINK: &str = "secret.link";
/// Params: [`SecretName`]. Result: [`SecretRemoved`].
pub const UNLINK: &str = "secret.unlink";
/// Params: [`SecretRemove`]. Result: [`SecretRemoved`].
pub const REMOVE: &str = "secret.remove";
/// Params: [`SecretRequestParams`]. Result: [`SecretRequest`].
pub const REQUEST: &str = "secret.request";
/// Params: [`SecretRequestsParams`]. Result: [`SecretRequests`].
pub const REQUESTS: &str = "secret.requests";
/// Params: [`SecretDecline`]. Result: [`SecretRequest`].
pub const DECLINE: &str = "secret.decline";
/// Params: none. Result: [`SecretStatus`]. Forgets the key of a vault that holds no secret.
pub const RESET_KEY: &str = "secret.reset_key";

/// Result of `secret.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretStatus {
    /// Where the vault key is kept: `dpapi`, `secret_service` or `file`.
    pub protection: String,
    /// `ready`, or `locked` when the key will not open.
    pub state: String,
    /// Owned secrets.
    pub count: u64,
}

/// Params of `secret.list`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// One name in one scope: owned, or a link to the secret of the same name in `owner_scope`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretEntry {
    pub scope: String,
    pub name: String,
    /// The scope that owns the value; equal to `scope` for an owned secret.
    pub owner_scope: String,
    pub linked: bool,
    /// Scopes that link to this owned secret; empty for a link.
    #[serde(default)]
    pub linked_scopes: Vec<String>,
    pub created_at_ms: i64,
    pub rotated_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_at_ms: Option<i64>,
}

/// Result of `secret.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretList {
    pub secrets: Vec<SecretEntry>,
}

/// Params of `secret.set`. `Debug` never prints the value.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretSet {
    pub scope: String,
    pub name: String,
    pub value: String,
    /// Rotates an owned secret that already has this name.
    #[serde(default)]
    pub replace: bool,
}

impl fmt::Debug for SecretSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretSet")
            .field("scope", &self.scope)
            .field("name", &self.name)
            .field("value", &"[redacted]")
            .field("replace", &self.replace)
            .finish()
    }
}

/// Params of `secret.link`: link `name` in `scope` to the secret of the same name owned by `from_scope`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretLink {
    pub scope: String,
    pub name: String,
    pub from_scope: String,
}

/// Params of `secret.unlink`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretName {
    pub scope: String,
    pub name: String,
}

/// Params of `secret.remove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRemove {
    pub scope: String,
    pub name: String,
    /// Also removes every scope's link to it; otherwise linked secrets are refused with `secret.has_links`.
    #[serde(default)]
    pub include_links: bool,
    /// Removes a secret while the vault is locked, without opening it.
    #[serde(default)]
    pub force: bool,
}

/// Result of `secret.unlink` and `secret.remove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRemoved {
    pub scope: String,
    pub name: String,
    /// Scopes whose links went with the secret.
    #[serde(default)]
    pub links_removed: Vec<String>,
}

/// Params of `secret.request`. Name and reason are untrusted text and are validated.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRequestParams {
    pub scope: String,
    pub name: String,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The run token of agent session `session` (`MASSORCA_AGENT_TOKEN`): when it matches a live run, the request
    /// counts against that agent's own quota instead of the connection's. `Debug` never prints it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_token: Option<String>,
}

impl fmt::Debug for SecretRequestParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretRequestParams")
            .field("scope", &self.scope)
            .field("name", &self.name)
            .field("reason", &self.reason)
            .field("session", &self.session)
            .field(
                "agent_token",
                &self.agent_token.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

/// Params of `secret.requests`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRequestsParams {
    /// Includes resolved requests; otherwise only pending ones.
    #[serde(default)]
    pub all: bool,
}

/// Params of `secret.decline`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretDecline {
    pub request: String,
}

/// One request for a secret by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRequest {
    pub id: String,
    pub scope: String,
    pub name: String,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// `pending`, `stored`, `linked`, `declined` or `failed`.
    pub state: String,
    pub created_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at_ms: Option<i64>,
}

/// Result of `secret.requests`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRequests {
    pub requests: Vec<SecretRequest>,
}

#[cfg(test)]
mod tests {
    use super::SecretSet;

    #[test]
    fn debug_never_prints_the_value() {
        let params = SecretSet {
            scope: "global".to_owned(),
            name: "TOKEN".to_owned(),
            value: "hunter2-value".to_owned(),
            replace: false,
        };
        let printed = format!("{params:?}");
        assert!(!printed.contains("hunter2"), "{printed}");
        assert!(printed.contains("[redacted]"), "{printed}");
    }
}
