//! `massorca host status`: connects to hosts and reports whether each one answers (decision 0046).

use std::time::Duration;

use massorca_client::{Client, ClientError, HostRecord, Hosts};
use massorca_profile::Profile;
use massorca_protocol::methods::devices::{self, WhoAmI};
use serde::Serialize;
use tokio::task::JoinSet;

use crate::connect::peer;
use crate::error::CliError;
use crate::output::Output;

/// How long one host may take to answer a status check, end to end.
const STATUS_TIMEOUT: Duration = Duration::from_secs(10);

/// A host that answered: its server version, and the scopes this device holds there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connected {
    pub server_version: String,
    pub scopes: Vec<String>,
}

/// One host's state as `host status` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostStatus {
    pub name: String,
    /// `connected`, `unreachable`, `refused`, `identity_mismatch` or `error`.
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    /// Why the host did not answer: the reason, or the refusal code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Every host's state, in name order.
#[derive(Debug, Serialize)]
pub struct StatusList {
    pub hosts: Vec<HostStatus>,
}

/// `host status [NAME]`: a named host must answer or the command fails; without a name, every host is reported.
pub async fn execute(
    profile: &Profile,
    name: Option<&str>,
    output: Output,
) -> Result<(), CliError> {
    let hosts = Hosts::of(profile);
    match name {
        Some(name) => {
            hosts.get(name)?;
            let connected = check(&hosts, name).await?;
            let status = status_of(name, Ok(connected));
            let line = status_line(&status);
            output.success(&status, &[line]);
            Ok(())
        }
        None => {
            let records = hosts.list()?;
            if records.is_empty() {
                output.success(&StatusList { hosts: Vec::new() }, &["no hosts".to_owned()]);
                return Ok(());
            }
            let statuses = check_all(&hosts, records).await?;
            let lines: Vec<String> = statuses.iter().map(status_line).collect();
            output.success(&StatusList { hosts: statuses }, &lines);
            Ok(())
        }
    }
}

/// Connects to `name` as this device and reads its scopes, within `STATUS_TIMEOUT`.
pub async fn check(hosts: &Hosts, name: &str) -> Result<Connected, ClientError> {
    tokio::time::timeout(STATUS_TIMEOUT, probe(hosts, name))
        .await
        .unwrap_or_else(|_| {
            Err(ClientError::HostUnreachable(format!(
                "no answer within {} seconds",
                STATUS_TIMEOUT.as_secs()
            )))
        })
}

async fn probe(hosts: &Hosts, name: &str) -> Result<Connected, ClientError> {
    let client = Client::connect_host(hosts, name, &peer()).await?;
    let me: WhoAmI = client.call(devices::WHOAMI, &()).await?;
    Ok(Connected {
        server_version: client.welcome().server.version.clone(),
        scopes: me.scopes,
    })
}

/// The state of one host from its check.
pub fn status_of(name: &str, result: Result<Connected, ClientError>) -> HostStatus {
    let mut status = HostStatus {
        name: name.to_owned(),
        state: "error",
        server_version: None,
        scopes: None,
        detail: None,
    };
    match result {
        Ok(connected) => {
            status.state = "connected";
            status.server_version = Some(connected.server_version);
            status.scopes = Some(connected.scopes);
        }
        Err(ClientError::ServerIdentityMismatch) => status.state = "identity_mismatch",
        Err(ClientError::HostUnreachable(reason)) => {
            status.state = "unreachable";
            status.detail = Some(reason);
        }
        Err(ClientError::Disconnected) => {
            status.state = "unreachable";
            status.detail = Some("the connection was lost".to_owned());
        }
        Err(ClientError::Handshake { error, .. }) => {
            status.state = "refused";
            status.detail = Some(error.code);
        }
        Err(error) => status.detail = Some(error.to_string()),
    }
    status
}

/// One host as a text line.
pub fn status_line(status: &HostStatus) -> String {
    let name = &status.name;
    match status.state {
        "connected" => {
            let scopes = status.scopes.as_deref().unwrap_or_default().join(", ");
            let version = status.server_version.as_deref().unwrap_or("unknown");
            format!("{name}: connected, server {version}, scopes {scopes}")
        }
        "identity_mismatch" => {
            format!("{name}: identity mismatch: the server is not the one paired with")
        }
        state => format!(
            "{name}: {state} ({})",
            status.detail.as_deref().unwrap_or("no detail")
        ),
    }
}

/// Checks every host at once, so one unreachable host does not hold up the others.
async fn check_all(hosts: &Hosts, records: Vec<HostRecord>) -> Result<Vec<HostStatus>, CliError> {
    let mut checks = JoinSet::new();
    for record in records {
        let hosts = hosts.clone();
        checks.spawn(async move {
            let result = check(&hosts, &record.name).await;
            status_of(&record.name, result)
        });
    }
    let mut statuses = Vec::new();
    while let Some(joined) = checks.join_next().await {
        statuses.push(joined?);
    }
    statuses.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(statuses)
}
