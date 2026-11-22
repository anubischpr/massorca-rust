//! Paired devices, pairing offers and the network state (`devices/1`, `network/1`; decision 0041).

use std::fmt;

use serde::{Deserialize, Serialize};

pub const LIST: &str = "device.list";
pub const REVOKE: &str = "device.revoke";
pub const WHOAMI: &str = "device.whoami";
pub const PAIRING_CREATE: &str = "pairing.create";
pub const NETWORK_STATUS: &str = "network.status";

/// Permission classes a device may hold, as they appear on the wire.
pub mod scopes {
    pub const READ: &str = "read";
    pub const WORKSPACE: &str = "workspace";
    pub const HOST_ADMIN: &str = "host-admin";
    pub const DEVICE_ADMIN: &str = "device-admin";
}

/// A string that is a credential. `Debug` never prints it, and it has no `Display`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The credential itself, for the one place that must send or compare it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// One paired device, as the server records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub scopes: Vec<String>,
    /// `sha256:<base64url>` of the device's public key.
    pub key_pin: String,
    pub paired_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at_ms: Option<i64>,
    /// Whether this is the device making the call.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceList {
    pub devices: Vec<DeviceInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRevokeParams {
    /// The device id, or its name when that is unique.
    pub device: String,
}

/// Who the caller is: the local owner or a paired device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhoAmI {
    /// `owner` or `device`.
    pub principal: String,
    pub scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<DeviceInfo>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingCreateParams {
    /// Lifetime in seconds; 600 when absent, at most 3600.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_secs: Option<u32>,
    /// Adds `host-admin` and `device-admin` to the default `read` and `workspace`.
    #[serde(default)]
    pub admin: bool,
}

/// A pairing offer. `offer` is the credential to hand to the new device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingCreated {
    pub offer: Secret,
    pub expires_at_ms: i64,
    pub scopes: Vec<String>,
    pub addresses: Vec<String>,
    pub server_pin: String,
}

/// One network listener of the service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListenerInfo {
    /// What was asked for, for example `tailscale:7347`.
    pub requested: String,
    /// The address it is bound to, once bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound: Option<String>,
    /// `listening`, `waiting` (the address is not there yet) or `failed`.
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub listeners: Vec<ListenerInfo>,
    /// Present once the service has a network identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_pin: Option<String>,
    pub devices: u32,
    pub pending_offers: u32,
}
