//! Typed helpers over the `services/1` methods (SKL-35). The token travels in `connect` only and no answer carries it.

use massorca_protocol::methods::hosted_services::{
    self, ServiceConnect, ServiceList, ServiceName, ServiceStatus, ServiceStatusParams,
};

use crate::client::Client;
use crate::error::ClientError;

impl Client {
    /// `marketplace.service.status`: one service, or every one.
    pub async fn hosted_service_status(
        &self,
        params: &ServiceStatusParams,
    ) -> Result<ServiceList, ClientError> {
        self.call(hosted_services::STATUS, params).await
    }

    /// `marketplace.service.connect`: stores the token on the host, or signs `glab` in with it.
    pub async fn hosted_service_connect(
        &self,
        params: &ServiceConnect,
    ) -> Result<ServiceStatus, ClientError> {
        self.call(hosted_services::CONNECT, params).await
    }

    /// `marketplace.service.disconnect`: deletes what MassOrca stored for the service.
    pub async fn hosted_service_disconnect(
        &self,
        params: &ServiceName,
    ) -> Result<ServiceStatus, ClientError> {
        self.call(hosted_services::DISCONNECT, params).await
    }
}
