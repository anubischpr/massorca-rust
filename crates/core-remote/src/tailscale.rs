//! Reads this node's Tailscale state through the local `tailscale` CLI (decision 0043).

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;

use crate::process::{self, CaptureError};

/// The CLI gets this long to print its status before it is stopped.
const STATUS_TIMEOUT: Duration = Duration::from_secs(5);

/// This node and the peers it can see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscaleStatus {
    pub backend_state: String,
    pub self_node: TailnetNode,
    /// Sorted by host name.
    pub peers: Vec<TailnetNode>,
    /// The tailnet's MagicDNS suffix, without a trailing dot.
    pub magic_dns_suffix: Option<String>,
}

/// One machine on the tailnet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailnetNode {
    pub host_name: String,
    /// The MagicDNS name, without its trailing dot.
    pub dns_name: Option<String>,
    pub addresses: Vec<IpAddr>,
    pub os: Option<String>,
    pub online: bool,
}

impl TailnetNode {
    /// The first IPv4 address of the node, if it has one.
    pub fn ipv4(&self) -> Option<Ipv4Addr> {
        self.addresses.iter().find_map(|address| match address {
            IpAddr::V4(v4) => Some(*v4),
            IpAddr::V6(_) => None,
        })
    }
}

/// Why no tailnet can be described. Each case is a normal answer, never a crash.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TailscaleUnavailable {
    #[error("Tailscale is not installed on this computer.")]
    NotInstalled,
    #[error("Tailscale is not connected (state: {backend_state}).")]
    NotRunning { backend_state: String },
    #[error("Tailscale could not be read: {0}")]
    Failed(String),
}

/// Reads the JSON printed by `tailscale status --json`. Unknown fields are ignored.
pub fn parse_status(json: &[u8]) -> Result<TailscaleStatus, TailscaleUnavailable> {
    let raw: RawStatus = serde_json::from_slice(json)
        .map_err(|err| TailscaleUnavailable::Failed(format!("the status is not JSON: {err}")))?;
    let backend_state = raw
        .backend_state
        .ok_or_else(|| TailscaleUnavailable::Failed("the status has no BackendState".to_owned()))?;
    if backend_state != "Running" {
        return Err(TailscaleUnavailable::NotRunning { backend_state });
    }
    let self_node = raw
        .self_node
        .ok_or_else(|| TailscaleUnavailable::Failed("the status has no Self node".to_owned()))?;
    let mut peers: Vec<TailnetNode> = raw
        .peer
        .unwrap_or_default()
        .into_values()
        .flatten()
        .map(TailnetNode::from)
        .collect();
    peers.sort_by(|a, b| a.host_name.cmp(&b.host_name));
    let magic_dns_suffix = raw
        .current_tailnet
        .and_then(|tailnet| clean_name(tailnet.magic_dns_suffix))
        .or_else(|| clean_name(raw.magic_dns_suffix));
    Ok(TailscaleStatus {
        backend_state,
        self_node: TailnetNode::from(self_node),
        peers,
        magic_dns_suffix,
    })
}

/// Finds the `tailscale` CLI on `PATH`, then in its standard install locations.
pub fn find_cli() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "tailscale.exe"
    } else {
        "tailscale"
    };
    process::find_on_path(name).or_else(|| process::first_executable(STANDARD_LOCATIONS))
}

#[cfg(windows)]
const STANDARD_LOCATIONS: &[&str] = &[r"C:\Program Files\Tailscale\tailscale.exe"];
#[cfg(target_os = "macos")]
const STANDARD_LOCATIONS: &[&str] = &[
    "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
    "/usr/local/bin/tailscale",
];
#[cfg(not(any(windows, target_os = "macos")))]
const STANDARD_LOCATIONS: &[&str] = &[];

/// Reads `tailscale status --json`. A missing CLI or a stopped daemon is an answer, not an error.
pub fn status() -> Result<TailscaleStatus, TailscaleUnavailable> {
    let cli = find_cli().ok_or(TailscaleUnavailable::NotInstalled)?;
    let mut command = massorca_process::command(&cli);
    command.args(["status", "--json"]);
    let captured = process::run_captured(command, None, STATUS_TIMEOUT).map_err(|err| {
        TailscaleUnavailable::Failed(match err {
            CaptureError::Spawn(err) => format!("could not run {}: {err}", cli.display()),
            CaptureError::Wait(err) => format!("could not wait for tailscale: {err}"),
            CaptureError::TimedOut => format!(
                "tailscale did not answer within {} s",
                STATUS_TIMEOUT.as_secs()
            ),
        })
    })?;
    read_status_output(
        &captured.stdout,
        captured.status.success(),
        &captured.stderr,
    )
}

/// A status JSON wins even from a non-zero exit, because a stopped daemon reports itself that way.
fn read_status_output(
    stdout: &[u8],
    succeeded: bool,
    stderr: &[u8],
) -> Result<TailscaleStatus, TailscaleUnavailable> {
    match parse_status(stdout) {
        Ok(status) => Ok(status),
        Err(not_running @ TailscaleUnavailable::NotRunning { .. }) => Err(not_running),
        Err(_) if !succeeded => Err(TailscaleUnavailable::Failed(process::excerpt(stderr))),
        Err(err) => Err(err),
    }
}

/// Drops empty names and the trailing dot of fully qualified ones.
fn clean_name(name: Option<String>) -> Option<String> {
    name.map(|name| name.trim_end_matches('.').to_owned())
        .filter(|name| !name.is_empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawStatus {
    backend_state: Option<String>,
    #[serde(rename = "Self")]
    self_node: Option<RawNode>,
    peer: Option<BTreeMap<String, Option<RawNode>>>,
    current_tailnet: Option<RawTailnet>,
    #[serde(rename = "MagicDNSSuffix")]
    magic_dns_suffix: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawNode {
    host_name: Option<String>,
    #[serde(rename = "DNSName")]
    dns_name: Option<String>,
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Option<Vec<String>>,
    #[serde(rename = "OS")]
    os: Option<String>,
    online: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawTailnet {
    #[serde(rename = "MagicDNSSuffix")]
    magic_dns_suffix: Option<String>,
}

impl From<RawNode> for TailnetNode {
    fn from(raw: RawNode) -> Self {
        Self {
            host_name: raw.host_name.unwrap_or_default(),
            dns_name: clean_name(raw.dns_name),
            addresses: raw
                .tailscale_ips
                .unwrap_or_default()
                .into_iter()
                .filter_map(|ip| ip.parse::<IpAddr>().ok())
                .collect(),
            os: raw.os.filter(|os| !os.is_empty()),
            online: raw.online.unwrap_or(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STOPPED: &[u8] = br#"{"BackendState":"Stopped"}"#;
    const RUNNING: &[u8] = br#"{"BackendState":"Running","Self":{"HostName":"node-self"}}"#;

    #[test]
    fn a_stopped_status_is_read_even_from_a_non_zero_exit() {
        assert_eq!(
            read_status_output(STOPPED, false, b""),
            Err(TailscaleUnavailable::NotRunning {
                backend_state: "Stopped".to_owned()
            })
        );
    }

    #[test]
    fn a_failed_run_without_status_reports_its_error_text() {
        assert_eq!(
            read_status_output(b"", false, b"failed to connect to tailscaled\n"),
            Err(TailscaleUnavailable::Failed(
                "failed to connect to tailscaled".to_owned()
            ))
        );
    }

    #[test]
    fn a_successful_run_with_garbage_output_is_a_failure() {
        assert!(matches!(
            read_status_output(b"not json", true, b""),
            Err(TailscaleUnavailable::Failed(_))
        ));
    }

    #[test]
    fn a_successful_run_with_a_running_status_is_ok() {
        assert!(read_status_output(RUNNING, true, b"").is_ok());
    }
}
