//! `massorca host discover`: the online Tailscale peers, and which of them answer on the MassOrca port (decision 0043).

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use massorca_client::Hosts;
use massorca_core_remote::tailscale::{self, TailnetNode};
use massorca_net::DEFAULT_PORT;
use massorca_profile::Profile;
use serde::Serialize;

use crate::error::CliError;
use crate::output::Output;
use crate::workspace_text;

/// How long a peer may take to accept a TCP connection on the MassOrca port.
const PROBE_TIMEOUT: Duration = Duration::from_millis(800);

/// What discovery found: the peers, or why there is no tailnet to look at.
#[derive(Debug, Serialize)]
pub struct Discovery {
    pub tailnet: bool,
    pub reason: Option<String>,
    pub peers: Vec<Peer>,
}

/// One online peer with an IPv4 address.
#[derive(Debug, Serialize)]
pub struct Peer {
    pub host_name: String,
    pub dns_name: Option<String>,
    pub ip: String,
    pub os: Option<String>,
    /// Whether a TCP connection to the MassOrca port was accepted.
    pub massorca: bool,
    /// The paired host at this address, when there is one.
    pub host: Option<String>,
}

/// `host discover`: lists the online peers, probing each one's MassOrca port in parallel.
pub async fn execute(profile: &Profile, output: Output) -> Result<(), CliError> {
    let status = match tokio::task::spawn_blocking(tailscale::status).await? {
        Ok(status) => status,
        Err(reason) => {
            let reason = reason.to_string();
            let discovery = Discovery {
                tailnet: false,
                reason: Some(reason.clone()),
                peers: Vec::new(),
            };
            output.success(&discovery, &[reason]);
            return Ok(());
        }
    };
    let known = known_addresses(profile)?;
    let online: Vec<(&TailnetNode, Ipv4Addr)> = status
        .peers
        .iter()
        .filter(|peer| peer.online)
        .filter_map(|peer| peer.ipv4().map(|ip| (peer, ip)))
        .collect();
    let probes: Vec<_> = online
        .iter()
        .map(|(_, ip)| {
            let ip = *ip;
            tokio::task::spawn_blocking(move || accepts(ip))
        })
        .collect();
    let mut peers = Vec::with_capacity(online.len());
    for ((node, ip), probe) in online.iter().zip(probes) {
        peers.push(Peer {
            host_name: node.host_name.clone(),
            dns_name: node.dns_name.clone(),
            ip: ip.to_string(),
            os: node.os.clone(),
            massorca: probe.await?,
            host: host_at(&known, *ip),
        });
    }
    let lines = peer_lines(&peers);
    let discovery = Discovery {
        tailnet: true,
        reason: None,
        peers,
    };
    output.success(&discovery, &lines);
    Ok(())
}

/// Whether a TCP connection to the MassOrca port of `ip` is accepted within `PROBE_TIMEOUT`.
fn accepts(ip: Ipv4Addr) -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::new(IpAddr::V4(ip), DEFAULT_PORT),
        PROBE_TIMEOUT,
    )
    .is_ok()
}

/// The IP address of every address a paired host was given, with the host's name.
fn known_addresses(profile: &Profile) -> Result<Vec<(IpAddr, String)>, CliError> {
    let mut known = Vec::new();
    for record in Hosts::of(profile).list()? {
        for address in &record.addresses {
            if let Ok(socket) = address.parse::<SocketAddr>() {
                known.push((socket.ip(), record.name.clone()));
            }
        }
    }
    Ok(known)
}

fn host_at(known: &[(IpAddr, String)], ip: Ipv4Addr) -> Option<String> {
    known
        .iter()
        .find(|(address, _)| *address == IpAddr::V4(ip))
        .map(|(_, name)| name.clone())
}

fn peer_lines(peers: &[Peer]) -> Vec<String> {
    if peers.is_empty() {
        return vec!["no online peers".to_owned()];
    }
    let rows = peers
        .iter()
        .map(|peer| {
            let massorca = if peer.massorca { "listening" } else { "no" };
            vec![
                peer.host_name.clone(),
                peer.dns_name.clone().unwrap_or_else(|| "-".to_owned()),
                peer.ip.clone(),
                peer.os.clone().unwrap_or_else(|| "-".to_owned()),
                massorca.to_owned(),
                peer.host.clone().unwrap_or_else(|| "-".to_owned()),
            ]
        })
        .collect();
    workspace_text::table(&["NAME", "DNS NAME", "IP", "OS", "MASSORCA", "HOST"], rows)
}
