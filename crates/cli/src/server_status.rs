//! `massorca server status`: the service's network state, and this computer's Tailscale state (decisions 0042, 0043).

use std::net::{IpAddr, SocketAddr};

use massorca_core_remote::tailscale::{self, TailscaleStatus};
use massorca_protocol::methods::devices::{self, ListenerInfo, NetworkStatus};
use serde::Serialize;

use crate::connect::connect;
use crate::error::CliError;
use crate::output::Output;
use crate::target::Target;
use crate::workspace_text;

/// The service's network state, with this computer's tailnet when the command runs locally.
#[derive(Debug, Serialize)]
pub struct ServerStatus {
    pub network: NetworkStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tailnet: Option<Tailnet>,
}

/// This computer's Tailscale state (decision 0043).
#[derive(Debug, Serialize)]
pub struct Tailnet {
    pub available: bool,
    /// Why there is no tailnet to report.
    pub reason: Option<String>,
    pub backend_state: Option<String>,
    pub addresses: Vec<String>,
    pub dns_name: Option<String>,
    /// Whether a listener of the service is bound to one of this computer's tailnet addresses.
    pub listening_on_tailnet: bool,
}

/// Runs `server status` against the service `target` names.
pub async fn execute(target: &Target, autostart: bool, output: Output) -> Result<(), CliError> {
    let client = connect(target, autostart).await?;
    let network: NetworkStatus = client.call(devices::NETWORK_STATUS, &()).await?;
    let tailnet = if target.is_local() {
        Some(read_tailnet(&network.listeners).await?)
    } else {
        None
    };
    let status = ServerStatus { network, tailnet };
    let lines = status_lines(&status);
    output.success(&status, &lines);
    Ok(())
}

/// Reads this computer's Tailscale state on a blocking thread. No tailnet is an answer, not an error.
async fn read_tailnet(listeners: &[ListenerInfo]) -> Result<Tailnet, CliError> {
    match tokio::task::spawn_blocking(tailscale::status).await? {
        Ok(status) => Ok(tailnet_of(&status, listeners)),
        Err(reason) => Ok(Tailnet {
            available: false,
            reason: Some(reason.to_string()),
            backend_state: None,
            addresses: Vec::new(),
            dns_name: None,
            listening_on_tailnet: false,
        }),
    }
}

fn tailnet_of(status: &TailscaleStatus, listeners: &[ListenerInfo]) -> Tailnet {
    let own = &status.self_node.addresses;
    let listening_on_tailnet = listeners
        .iter()
        .filter_map(|listener| listener.bound.as_deref()?.parse::<SocketAddr>().ok())
        .any(|bound| own.contains(&bound.ip()));
    Tailnet {
        available: true,
        reason: None,
        backend_state: Some(status.backend_state.clone()),
        addresses: own.iter().map(IpAddr::to_string).collect(),
        dns_name: status.self_node.dns_name.clone(),
        listening_on_tailnet,
    }
}

fn status_lines(status: &ServerStatus) -> Vec<String> {
    let network = &status.network;
    let mut lines = if network.listeners.is_empty() {
        vec!["no network listeners: the service serves this computer only".to_owned()]
    } else {
        listener_table(&network.listeners)
    };
    lines.push(format!(
        "server pin: {}",
        network.server_pin.as_deref().unwrap_or("none")
    ));
    lines.push(format!("active devices: {}", network.devices));
    lines.push(format!("pending offers: {}", network.pending_offers));
    if let Some(tailnet) = &status.tailnet {
        lines.push(tailnet_line(tailnet));
    }
    lines
}

pub(crate) fn listener_table(listeners: &[ListenerInfo]) -> Vec<String> {
    let rows = listeners
        .iter()
        .map(|listener| {
            let state = match &listener.error {
                Some(error) => format!("{} ({error})", listener.state),
                None => listener.state.clone(),
            };
            vec![
                listener.requested.clone(),
                listener.bound.clone().unwrap_or_else(|| "-".to_owned()),
                state,
            ]
        })
        .collect();
    workspace_text::table(&["REQUESTED", "BOUND", "STATE"], rows)
}

fn tailnet_line(tailnet: &Tailnet) -> String {
    match &tailnet.reason {
        Some(reason) => format!("tailnet: {reason}"),
        None => format!(
            "tailnet: {} at {}; name {}; listening on it: {}",
            tailnet.backend_state.as_deref().unwrap_or("unknown"),
            tailnet.addresses.join(", "),
            tailnet.dns_name.as_deref().unwrap_or("none"),
            if tailnet.listening_on_tailnet {
                "yes"
            } else {
                "no"
            }
        ),
    }
}
