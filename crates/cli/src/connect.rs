//! Connecting to the service a command addresses: a paired host over the network, or the local service of a profile.

use std::path::PathBuf;

use massorca_client::{Autostart, Client, ClientError, ConnectOptions, Hosts};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;

use crate::error::{CliError, exit};
use crate::target::Target;

/// Who this command says it is in the handshake.
pub fn peer() -> PeerInfo {
    PeerInfo {
        name: "massorca".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

/// Connects to the service `target` names. A host is reached over the network and never started.
pub async fn connect(target: &Target, autostart: bool) -> Result<Client, CliError> {
    match &target.host {
        Some(name) => Ok(Client::connect_host(&Hosts::of(&target.profile), name, &peer()).await?),
        None => connect_local(&target.profile, autostart).await,
    }
}

/// Connects to the local service of `profile`, starting it first when nothing listens and `autostart` is set.
pub async fn connect_local(profile: &Profile, autostart: bool) -> Result<Client, CliError> {
    let mut options = ConnectOptions::new(peer());
    if autostart {
        options.autostart = Some(Autostart {
            exe: current_exe()?,
            args_extra: Vec::new(),
        });
    }
    Ok(Client::connect(profile, options).await?)
}

/// Connects without starting the service. `None` when nothing is listening.
pub async fn connect_running(profile: &Profile) -> Result<Option<Client>, CliError> {
    match Client::connect(profile, ConnectOptions::new(peer())).await {
        Ok(client) => Ok(Some(client)),
        Err(ClientError::Unavailable(_)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn current_exe() -> Result<PathBuf, CliError> {
    std::env::current_exe().map_err(|error| {
        CliError::new(
            "io.failed",
            format!("cannot locate the massorca executable: {error}"),
            exit::ERROR,
        )
    })
}
