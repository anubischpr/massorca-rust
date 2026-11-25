//! Hosted-service connections from the Marketplace (capability `services/1`, SKL-35, decision 0094): GitLab, Bitbucket,
//! Azure DevOps and Gitea connect and disconnect; Linear and Jira are status-only rows. A token travels inward once,
//! in `connect`, and no result carries it.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Params: [`ServiceStatusParams`]. Result: [`ServiceList`].
pub const STATUS: &str = "marketplace.service.status";
/// Params: [`ServiceConnect`]. Result: [`ServiceStatus`].
pub const CONNECT: &str = "marketplace.service.connect";
/// Params: [`ServiceName`]. Result: [`ServiceStatus`].
pub const DISCONNECT: &str = "marketplace.service.disconnect";

/// Every service id, in the order `status` lists them.
pub const SERVICE_IDS: [&str; 6] = [
    "gitlab",
    "bitbucket",
    "azure-devops",
    "gitea",
    "linear",
    "jira",
];

/// Params of `status`: one service, or every one when `service` is left out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStatusParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
}

/// Params of `connect`. `Debug` never prints the token.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceConnect {
    pub service: String,
    pub token: String,
    /// The server address when the service is self-hosted: a GitLab host, a Gitea server, an Azure DevOps API.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Bitbucket only: with an email the token is an Atlassian API token, without one an access token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

impl fmt::Debug for ServiceConnect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceConnect")
            .field("service", &self.service)
            .field("token", &"[redacted]")
            .field("base_url", &self.base_url)
            .field("email", &self.email)
            .finish()
    }
}

/// Params of `disconnect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceName {
    pub service: String,
}

/// Result of `status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceList {
    pub services: Vec<ServiceStatus>,
}

/// One service as the host reports it: never a token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub service: String,
    pub name: String,
    pub state: ServiceState,
    pub source: ServiceSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// When MassOrca last stored the connection; only for a `stored` source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_ms: Option<i64>,
    /// GitLab only: whether `glab` is on the host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_installed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceState {
    Connected,
    NotConnected,
    /// Also what a state from a newer peer decodes as.
    #[serde(other)]
    Unknown,
}

/// Where the connection comes from: saved by MassOrca, set up outside it, or nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceSource {
    Stored,
    External,
    None,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}
