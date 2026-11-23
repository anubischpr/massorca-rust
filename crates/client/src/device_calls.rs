//! Typed helpers over the `devices/1` and `network/1` methods (decision 0041): the paired devices, pairing offers and
//! the network listeners of the service.

use massorca_protocol::methods::devices::{
    self, DeviceForgetParams, DeviceGrantParams, DeviceInfo, DeviceList, DeviceRevokeParams,
    NetworkStatus, PairingCreateParams, PairingCreated,
};

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

    /// `pairing.create`: an offer another device can redeem once, before it expires.
    pub async fn pairing_create(
        &self,
        params: &PairingCreateParams,
    ) -> Result<PairingCreated, ClientError> {
        self.call(devices::PAIRING_CREATE, params).await
    }

    /// `network.status`: the listeners, the server pin, and how many devices and offers there are.
    pub async fn network_status(&self) -> Result<NetworkStatus, ClientError> {
        self.call(devices::NETWORK_STATUS, &()).await
    }
}
