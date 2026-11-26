//! `massorca host status --watch`: keeps a live link to each host (REM-07) and prints its state each time it changes,
//! until interrupted. A lost host is retried with the network backoff; a refusal or a changed key is not.

use std::time::Duration;

use massorca_client::{Backoff, HostDial, Hosts, LiveHost, LiveState};
use massorca_profile::Profile;
use serde::Serialize;
use tokio::sync::mpsc;

use super::status::{HostStatus, StatusList, failure_status, status_line};
use crate::connect::peer;
use crate::error::CliError;
use crate::output::Output;

/// One host's state as a watch prints it: the `host status` fields, and when an unreachable host is tried next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WatchedStatus {
    #[serde(flatten)]
    pub status: HostStatus,
    /// The attempt that runs next, from 1, while unreachable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_in_ms: Option<u64>,
}

/// `host status --watch [NAME]`: one line per change, a JSON document per line with `--json`.
pub async fn execute(
    profile: &Profile,
    name: Option<&str>,
    output: Output,
) -> Result<(), CliError> {
    let hosts = Hosts::of(profile);
    let names = match name {
        Some(name) => vec![hosts.get(name)?.name],
        None => hosts
            .list()?
            .into_iter()
            .map(|record| record.name)
            .collect(),
    };
    if names.is_empty() {
        output.success(&StatusList { hosts: Vec::new() }, &["no hosts".to_owned()]);
        return Ok(());
    }
    let (changes, mut changed) = mpsc::unbounded_channel();
    // Kept until the command ends: dropping a link stops it.
    let mut links = Vec::new();
    for name in names {
        let link = LiveHost::start(
            HostDial::new(hosts.clone(), name.clone(), peer()),
            Backoff::default(),
        );
        let mut states = link.subscribe();
        let changes = changes.clone();
        tokio::spawn(async move {
            loop {
                let state = states.borrow_and_update().clone();
                if changes.send(watched(&name, &state)).is_err() || states.changed().await.is_err()
                {
                    return;
                }
            }
        });
        links.push(link);
    }
    drop(changes);
    while let Some(status) = changed.recv().await {
        output.success(&status, &[watch_line(&status)]);
    }
    Ok(())
}

/// A link state in the words of `host status`.
pub fn watched(name: &str, state: &LiveState) -> WatchedStatus {
    let mut watched = WatchedStatus {
        status: HostStatus::named(name, "connecting"),
        attempt: None,
        retry_in_ms: None,
    };
    match state {
        LiveState::Connecting => {}
        LiveState::Connected { version, .. } => {
            watched.status.state = "connected";
            watched.status.server_version = Some(version.clone());
        }
        LiveState::Unreachable {
            error,
            attempt,
            retry_in,
        } => {
            // Every error the link retries is a loss of contact, whatever its kind.
            watched.status = HostStatus {
                state: "unreachable",
                ..failure_status(name, error)
            };
            watched.attempt = Some(*attempt);
            watched.retry_in_ms = Some(u64::try_from(retry_in.as_millis()).unwrap_or(u64::MAX));
        }
        LiveState::Ended { error } => watched.status = failure_status(name, error),
    }
    watched
}

/// One change as a text line.
pub fn watch_line(watched: &WatchedStatus) -> String {
    let status = &watched.status;
    let name = &status.name;
    match (status.state, watched.attempt, watched.retry_in_ms) {
        ("connecting", ..) => format!("{name}: connecting"),
        ("connected", ..) => format!(
            "{name}: connected, server {}",
            status.server_version.as_deref().unwrap_or("unknown")
        ),
        ("unreachable", Some(attempt), Some(retry_in_ms)) => format!(
            "{name}: unreachable ({}); attempt {attempt} in {:.1} s",
            status.detail.as_deref().unwrap_or("no detail"),
            Duration::from_millis(retry_in_ms).as_secs_f64()
        ),
        _ => status_line(status),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use massorca_client::ClientError;

    use super::*;

    #[test]
    fn each_state_has_its_line() {
        let connected = watched(
            "vps",
            &LiveState::Connected {
                server: "massorca".into(),
                version: "0.1.0".into(),
            },
        );
        assert_eq!(watch_line(&connected), "vps: connected, server 0.1.0");
        assert_eq!(
            watch_line(&watched("vps", &LiveState::Connecting)),
            "vps: connecting"
        );
        let lost = watched(
            "vps",
            &LiveState::Unreachable {
                error: Arc::new(ClientError::Disconnected),
                attempt: 2,
                retry_in: Duration::from_millis(1500),
            },
        );
        assert_eq!(
            watch_line(&lost),
            "vps: unreachable (the connection was lost); attempt 2 in 1.5 s"
        );
        assert_eq!((lost.attempt, lost.retry_in_ms), (Some(2), Some(1500)));
        let ended = watched(
            "vps",
            &LiveState::Ended {
                error: Arc::new(ClientError::HostNotFound("vps".into())),
            },
        );
        assert_eq!(ended.status.state, "error");
        assert_eq!(ended.attempt, None);
    }
}
