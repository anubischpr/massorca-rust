//! What a started service reports once it accepts connections: its local endpoint, and with a network listener, the
//! endpoints it binds and offers advertise, its server pin and its pending pairing offers. Never an offer's secret.

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

/// The network part of a readiness record: the listener states and pin `devices.network_status` reports, the endpoints
/// pairing offers advertise, and the pending offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkReadiness {
    pub listeners: Vec<ListenerInfo>,
    pub server_pin: String,
    /// The addresses the listeners are bound to, `host:port`.
    pub bound_endpoints: Vec<String>,
    /// What an offer lists when it names no address: the bound listeners and their MagicDNS names.
    pub advertised_endpoints: Vec<String>,
    pub pending_offers: Vec<PendingOffer>,
}

/// A pending pairing offer: the id that names it, when it ends in wall-clock milliseconds, and its scopes. No secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingOffer {
    pub offer_id: String,
    pub expires_at_ms: i64,
    pub scopes: Vec<String>,
}

impl Readiness {
    pub(crate) fn of(profile: &Profile, network: &NetworkState) -> Self {
        Self {
            local_endpoint: local_endpoint(profile),
            network: network.server_pin.map(|pin| {
                let listeners = network.listener_infos();
                NetworkReadiness {
                    bound_endpoints: listeners
                        .iter()
                        .filter_map(|listener| listener.bound.clone())
                        .collect(),
                    listeners,
                    server_pin: pin.to_string(),
                    advertised_endpoints: network.advertised_addresses(),
                    pending_offers: network
                        .pairing
                        .pending_offers(Instant::now())
                        .into_iter()
                        .map(|offer| PendingOffer {
                            offer_id: offer.offer_id,
                            expires_at_ms: offer.expires_at_ms,
                            scopes: offer.scopes.names(),
                        })
                        .collect(),
                }
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
