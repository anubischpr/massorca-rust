//! What a started service reports once it accepts connections: its local endpoint, and with a network listener, the
//! listeners, the server pin and how many pairing offers are pending. Never an offer's secret.

use std::time::Instant;

use massorca_profile::Profile;
use massorca_protocol::methods::devices::ListenerInfo;

use crate::network::NetworkState;

/// The readiness record of a service, read once its listeners have settled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readiness {
    /// Where local clients connect: the socket file, or the named pipe on Windows.
    pub local_endpoint: String,
    /// `None` when the service has no network listener.
    pub network: Option<NetworkReadiness>,
}

/// The network part of a readiness record: the same listener states and pin `devices.network_status` reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkReadiness {
    pub listeners: Vec<ListenerInfo>,
    pub server_pin: String,
    pub pending_offers: u32,
}

impl Readiness {
    pub(crate) fn of(profile: &Profile, network: &NetworkState) -> Self {
        Self {
            local_endpoint: local_endpoint(profile),
            network: network.server_pin.map(|pin| NetworkReadiness {
                listeners: network.listener_infos(),
                server_pin: pin.to_string(),
                pending_offers: u32::try_from(network.pairing.pending(Instant::now()))
                    .unwrap_or(u32::MAX),
            }),
        }
    }
}

#[cfg(unix)]
fn local_endpoint(profile: &Profile) -> String {
    profile.socket_path().display().to_string()
}

#[cfg(windows)]
fn local_endpoint(profile: &Profile) -> String {
    format!(r"\.\pipe\{}", profile.pipe_name())
}
