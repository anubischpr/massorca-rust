//! The readiness record `service run` prints once the service accepts connections (SRV-02): one JSON document, or text.

use massorca_protocol::methods::devices::ListenerInfo;
use massorca_protocol::methods::service::ServiceInfo;
use massorca_server::Readiness;
use serde::Serialize;

use crate::server_status::listener_table;

/// What a started service is and where it listens. The start identity is `ServiceInfo`'s, unchanged.
#[derive(Debug, Serialize)]
pub struct ReadinessRecord {
    #[serde(flatten)]
    pub info: ServiceInfo,
    pub endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkRecord>,
}

/// The network listeners, the server pin and the pending pairing offers (a count only, never an offer).
#[derive(Debug, Serialize)]
pub struct NetworkRecord {
    pub listeners: Vec<ListenerInfo>,
    pub server_pin: String,
    pub pending_offers: u32,
}

impl ReadinessRecord {
    pub fn new(info: ServiceInfo, ready: Readiness) -> Self {
        Self {
            info,
            endpoint: ready.local_endpoint,
            network: ready.network.map(|network| NetworkRecord {
                listeners: network.listeners,
                server_pin: network.server_pin,
                pending_offers: network.pending_offers,
            }),
        }
    }
}

/// The text form: the start line, then the endpoint and, with a network listener, its table, pin and offers.
pub fn lines(record: &ReadinessRecord) -> Vec<String> {
    let info = &record.info;
    let mut lines = vec![
        format!(
            "service ready for profile {} (pid {})",
            info.profile, info.pid
        ),
        format!("version {}, protocol {}", info.version, info.protocol),
        format!("local endpoint: {}", record.endpoint),
    ];
    match &record.network {
        None => {
            lines.push("no network listeners: the service serves this computer only".to_owned())
        }
        Some(network) => {
            lines.extend(listener_table(&network.listeners));
            lines.push(format!("server pin: {}", network.server_pin));
            lines.push(format!("pending offers: {}", network.pending_offers));
        }
    }
    lines
}
