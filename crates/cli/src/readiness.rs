//! The readiness record `service run` prints once the service accepts connections (SRV-02): one JSON document, or text.

use massorca_protocol::methods::devices::ListenerInfo;
use massorca_protocol::methods::service::ServiceInfo;
use massorca_server::Readiness;
use serde::Serialize;

use crate::server_status::listener_table;

/// The schema name of the record, and its version.
pub const SCHEMA: &str = "orca_server_ready";
pub const SCHEMA_VERSION: u32 = 1;

/// What a started service is, where it listens and what pairing can do now. The start identity is `ServiceInfo`'s.
#[derive(Debug, Serialize)]
pub struct ReadinessRecord {
    pub schema: &'static str,
    pub schema_version: u32,
    #[serde(flatten)]
    pub info: ServiceInfo,
    pub endpoint: String,
    pub bound_endpoints: Vec<String>,
    pub advertised_endpoints: Vec<String>,
    pub pairing: PairingRecord,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkRecord>,
}

/// The network listeners, the server pin and the pending offer count.
#[derive(Debug, Serialize)]
pub struct NetworkRecord {
    pub listeners: Vec<ListenerInfo>,
    pub server_pin: String,
    pub pending_offers: u32,
}

/// Whether pairing works now, why not, and the offers still pending (never their secrets).
#[derive(Debug, Serialize)]
pub struct PairingRecord {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub pending_offers: Vec<PendingOfferRecord>,
}

#[derive(Debug, Serialize)]
pub struct PendingOfferRecord {
    pub offer_id: String,
    pub expires_at_ms: i64,
    pub scopes: Vec<String>,
}

const NO_LISTENER_REASON: &str =
    "the service is not listening on the network; start it with --listen";
const NOT_BOUND_REASON: &str = "no network listener is bound yet; pairing needs a bound listener";

impl ReadinessRecord {
    pub fn new(info: ServiceInfo, ready: Readiness) -> Self {
        let endpoint = ready.local_endpoint;
        let Some(network) = ready.network else {
            return Self {
                schema: SCHEMA,
                schema_version: SCHEMA_VERSION,
                info,
                endpoint,
                bound_endpoints: Vec::new(),
                advertised_endpoints: Vec::new(),
                pairing: PairingRecord {
                    available: false,
                    reason: Some(NO_LISTENER_REASON.to_owned()),
                    pending_offers: Vec::new(),
                },
                network: None,
            };
        };
        let available = !network.advertised_endpoints.is_empty();
        let pending_count = u32::try_from(network.pending_offers.len()).unwrap_or(u32::MAX);
        Self {
            schema: SCHEMA,
            schema_version: SCHEMA_VERSION,
            info,
            endpoint,
            bound_endpoints: network.bound_endpoints,
            pairing: PairingRecord {
                available,
                reason: (!available).then(|| NOT_BOUND_REASON.to_owned()),
                pending_offers: network
                    .pending_offers
                    .into_iter()
                    .map(|offer| PendingOfferRecord {
                        offer_id: offer.offer_id,
                        expires_at_ms: offer.expires_at_ms,
                        scopes: offer.scopes,
                    })
                    .collect(),
            },
            advertised_endpoints: network.advertised_endpoints,
            network: Some(NetworkRecord {
                listeners: network.listeners,
                server_pin: network.server_pin,
                pending_offers: pending_count,
            }),
        }
    }
}

/// The text form: the start line, then the endpoints, the listener table and pin, and whether pairing works.
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
            lines.push(format!(
                "advertised endpoints: {}",
                list_or_none(&record.advertised_endpoints)
            ));
            lines.push(format!("pending offers: {}", network.pending_offers));
        }
    }
    match &record.pairing.reason {
        None => lines.push("pairing: available".to_owned()),
        Some(reason) => lines.push(format!("pairing: unavailable: {reason}")),
    }
    lines
}

fn list_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(", ")
    }
}
