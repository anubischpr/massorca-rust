//! The hosted services of a host (SKL-35, decision 0097): `marketplace.service.status`, `connect` and `disconnect`,
//! on this computer's service or on a paired host's live connection. A token goes out once, in `connect`; no answer
//! carries it, and nothing here keeps it.

use massorca_client::Client;
use massorca_protocol::methods::hosted_services::{
    ServiceConnect, ServiceList, ServiceName, ServiceStatus, ServiceStatusParams,
};
use tokio::sync::oneshot;

use super::bridge::{Command, ServiceHandle};
use super::live_hosts::LiveHosts;
use super::refusal::Refusal;

/// One hosted-service call, answered on its oneshot. `Debug` of the connect params never prints the token.
#[derive(Debug)]
pub(crate) enum ServicesCommand {
    Status {
        reply: oneshot::Sender<Result<ServiceList, Refusal>>,
    },
    Connect {
        params: ServiceConnect,
        reply: oneshot::Sender<Result<ServiceStatus, Refusal>>,
    },
    Disconnect {
        params: ServiceName,
        reply: oneshot::Sender<Result<ServiceStatus, Refusal>>,
    },
}

impl ServiceHandle {
    /// `marketplace.service.status` of every service on `host`, or on this computer when `host` is `None`.
    pub async fn hosted_services(&self, host: Option<String>) -> Result<ServiceList, Refusal> {
        self.ask(|reply| Command::HostedServices {
            host,
            command: ServicesCommand::Status { reply },
        })
        .await
    }

    /// `marketplace.service.connect` on `host`. The params, token included, are dropped once sent.
    pub async fn connect_hosted_service(
        &self,
        host: Option<String>,
        params: ServiceConnect,
    ) -> Result<ServiceStatus, Refusal> {
        self.ask(|reply| Command::HostedServices {
            host,
            command: ServicesCommand::Connect { params, reply },
        })
        .await
    }

    /// `marketplace.service.disconnect` on `host`.
    pub async fn disconnect_hosted_service(
        &self,
        host: Option<String>,
        service: String,
    ) -> Result<ServiceStatus, Refusal> {
        self.ask(|reply| Command::HostedServices {
            host,
            command: ServicesCommand::Disconnect {
                params: ServiceName { service },
                reply,
            },
        })
        .await
    }
}

/// Runs one call on this computer's service, on its own task.
pub(super) fn spawn_local(client: &Client, command: ServicesCommand) {
    let client = client.clone();
    tokio::spawn(async move { answer(&client, command).await });
}

/// Runs one call on a paired host's live connection; a host that cannot be reached refuses it.
pub(super) fn spawn_host(live: LiveHosts, host: String, command: ServicesCommand) {
    tokio::spawn(async move {
        match live.client(&host).await {
            Ok(client) => answer(&client, command).await,
            Err(error) => refuse_while_down(command, Refusal::from(&*error)),
        }
    });
}

async fn answer(client: &Client, command: ServicesCommand) {
    match command {
        ServicesCommand::Status { reply } => {
            let result = client
                .hosted_service_status(&ServiceStatusParams::default())
                .await;
            let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        }
        ServicesCommand::Connect { params, reply } => {
            let result = client.hosted_service_connect(&params).await;
            drop(params);
            let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        }
        ServicesCommand::Disconnect { params, reply } => {
            let result = client.hosted_service_disconnect(&params).await;
            let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
        }
    }
}

pub(super) fn refuse_while_down(command: ServicesCommand, refusal: Refusal) {
    match command {
        ServicesCommand::Status { reply } => {
            let _ = reply.send(Err(refusal));
        }
        ServicesCommand::Connect { reply, .. } | ServicesCommand::Disconnect { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
    }
}
