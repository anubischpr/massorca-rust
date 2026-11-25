//! Typed helpers over the `devices/1` and `network/1` methods (decision 0041): the paired devices, pairing offers and
//! the network listeners of the service.

use massorca_protocol::methods::devices::{
    self, DeviceForgetParams, DeviceGrantParams, DeviceInfo, DeviceList, DeviceRevokeParams,
    NetworkStatus, PairingCancelParams, PairingCancelled, PairingCreateParams, PairingCreated,
    WhoAmI,
};

use massorca_protocol::capabilities;

use crate::client::Client;
use crate::error::ClientError;

impl Client {
    /// `device.list`: every device paired with the service, revoked ones included.
    pub async fn device_list(&self) -> Result<DeviceList, ClientError> {
        self.call(devices::LIST, &()).await
    }

    /// `device.revoke`: revokes a device by id, or by name when that is unique, and answers with it.
    pub async fn device_revoke(
        &self,
        params: &DeviceRevokeParams,
    ) -> Result<DeviceInfo, ClientError> {
        self.call(devices::REVOKE, params).await
    }

    /// `device.forget`: removes a revoked device from the list, and answers with it. Needs host-admin.
    pub async fn device_forget(
        &self,
        params: &DeviceForgetParams,
    ) -> Result<DeviceInfo, ClientError> {
        self.call(devices::FORGET, params).await
    }

    /// `device.grant`: gives a device every scope, from its next connection. Needs host-admin.
    pub async fn device_grant(
        &self,
        params: &DeviceGrantParams,
    ) -> Result<DeviceInfo, ClientError> {
        self.call(devices::GRANT, params).await
    }

    /// `device.whoami`: the device this connection runs as and its scopes; no device for the local owner.
    pub async fn device_whoami(&self) -> Result<WhoAmI, ClientError> {
        self.call(devices::WHOAMI, &()).await
    }

    /// `pairing.create`: an offer another device can redeem once, before it expires. Given `addresses` are sent only
    /// to a service offering `pairing-address/1`; an older one would ignore them and list its bound addresses.
    pub async fn pairing_create(
        &self,
        params: &PairingCreateParams,
    ) -> Result<PairingCreated, ClientError> {
        if !params.addresses.is_empty() && !self.has_capability(capabilities::PAIRING_ADDRESS) {
            return Err(ClientError::CapabilityMissing {
                what: "pairing offer addresses".to_owned(),
                capability: capabilities::PAIRING_ADDRESS.to_owned(),
            });
        }
        self.call(devices::PAIRING_CREATE, params).await
    }

    /// `pairing.cancel`: retires one pending offer, or every one, before it expires. Sent only to a service offering
    /// `pairing-cancel/1`.
    pub async fn pairing_cancel(
        &self,
        params: &PairingCancelParams,
    ) -> Result<PairingCancelled, ClientError> {
        if !self.has_capability(capabilities::PAIRING_CANCEL) {
            return Err(ClientError::CapabilityMissing {
                what: "retiring a pairing offer".to_owned(),
                capability: capabilities::PAIRING_CANCEL.to_owned(),
            });
        }
        self.call(devices::PAIRING_CANCEL, params).await
    }

    /// `network.status`: the listeners, the server pin, and how many devices and offers there are.
    pub async fn network_status(&self) -> Result<NetworkStatus, ClientError> {
        self.call(devices::NETWORK_STATUS, &()).await
    }
}
