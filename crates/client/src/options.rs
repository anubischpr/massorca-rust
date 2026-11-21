//! What a client needs to connect: who it is, whether it may start the service, and how long to wait for a started service.

use std::path::PathBuf;
use std::time::Duration;

use massorca_protocol::PeerInfo;

/// How the client starts the service when nothing listens: `<exe> service run --profile <name> <args_extra...>`, detached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Autostart {
    pub exe: PathBuf,
    pub args_extra: Vec<String>,
}

/// Options of `Client::connect`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectOptions {
    /// Who the client says it is in `hello`.
    pub client: PeerInfo,
    /// Starts the service when nothing listens. `None` reports the service as unavailable instead.
    pub autostart: Option<Autostart>,
    /// How long a started service may take to accept connections.
    pub connect_timeout: Duration,
}

impl ConnectOptions {
    /// Options for `client` with no autostart and a 10 second connect timeout.
    pub fn new(client: PeerInfo) -> Self {
        Self {
            client,
            autostart: None,
            connect_timeout: Duration::from_secs(10),
        }
    }
}
