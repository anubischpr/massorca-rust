//! Connecting to a profile's service, and starting the service first when the options allow it (ADR 0003).

use std::io;
use std::time::Duration;

use interprocess::local_socket::Name;
use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::traits::tokio::Stream as _;
use massorca_profile::{Profile, local_socket_name};
use tokio::time::{Instant, sleep};

use crate::client::Client;
use crate::error::ClientError;
use crate::handshake;
use crate::options::ConnectOptions;
use crate::spawn;

/// The first pause between attempts to reach a starting service. It doubles up to `MAX_PAUSE`.
const INITIAL_PAUSE: Duration = Duration::from_millis(25);
const MAX_PAUSE: Duration = Duration::from_millis(250);

pub(crate) async fn connect(
    profile: &Profile,
    options: ConnectOptions,
) -> Result<Client, ClientError> {
    let name = local_socket_name(profile)?;
    let mut stream = match Stream::connect(name.clone()).await {
        Ok(stream) => stream,
        Err(error) if nobody_listens(&error) => match &options.autostart {
            Some(autostart) => {
                spawn::service_run(profile, autostart)?;
                wait_for_listener(name, options.connect_timeout).await?
            }
            None => return Err(ClientError::Unavailable(error)),
        },
        Err(error) => return Err(ClientError::Unavailable(error)),
    };
    let welcome = handshake::exchange(&mut stream, &options.client).await?;
    Ok(Client::open(stream, welcome))
}

/// Retries until the started service accepts a connection, or the timeout passes.
async fn wait_for_listener(name: Name<'static>, timeout: Duration) -> Result<Stream, ClientError> {
    let deadline = Instant::now() + timeout;
    let mut pause = INITIAL_PAUSE;
    loop {
        match Stream::connect(name.clone()).await {
            Ok(stream) => return Ok(stream),
            Err(error) if nobody_listens(&error) || error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(ClientError::Unavailable(error)),
        }
        if Instant::now() >= deadline {
            return Err(ClientError::StartFailed(format!(
                "the service did not accept connections within {timeout:?}"
            )));
        }
        sleep(pause).await;
        pause = (pause * 2).min(MAX_PAUSE);
    }
}

fn nobody_listens(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    )
}
