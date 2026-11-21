//! Typed helpers over the methods of the core features.

use massorca_protocol::methods::config::{self, ConfigKey, ConfigList, ConfigSet, ConfigValue};
use massorca_protocol::methods::events;
use massorca_protocol::methods::service::{self, ServiceInfo};

use crate::client::Client;
use crate::error::ClientError;
use crate::subscription::Subscription;

impl Client {
    /// `service.info`: the service's identity, protocol version, capabilities and process.
    pub async fn service_info(&self) -> Result<ServiceInfo, ClientError> {
        self.call(service::INFO, &()).await
    }

    /// `service.shutdown`: asks the service to stop. The request is answered before the service winds down.
    pub async fn shutdown(&self) -> Result<(), ClientError> {
        self.call(service::SHUTDOWN, &()).await
    }

    /// `events.subscribe`: the change events the service publishes.
    pub async fn subscribe_events(&self) -> Result<Subscription, ClientError> {
        self.subscribe(events::SUBSCRIBE, &()).await
    }

    /// `config.get`: one setting, its value and its default.
    pub async fn config_get(&self, params: &ConfigKey) -> Result<ConfigValue, ClientError> {
        self.call(config::GET, params).await
    }

    /// `config.set`: changes one setting after validating it.
    pub async fn config_set(&self, params: &ConfigSet) -> Result<ConfigValue, ClientError> {
        self.call(config::SET, params).await
    }

    /// `config.list`: every declared setting with its value.
    pub async fn config_list(&self) -> Result<ConfigList, ClientError> {
        self.call(config::LIST, &()).await
    }
}
