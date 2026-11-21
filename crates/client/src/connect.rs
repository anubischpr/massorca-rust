//! Connecting to a profile's service, and starting the service first when the options allow it (ADR 0003).

use std::io;
use std::time::Duration;

use interprocess::local_socket::Name;
use interprocess::local_socket::tokio::Stream;
use interprocess::local_socket::traits::StreamCommon as _;
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
    ensure_trusted_endpoint(&stream)?;
    let welcome = handshake::exchange(&mut stream, &options.client).await?;
    Ok(Client::open(stream, welcome))
}

/// Refuses an endpoint another user serves, or whose owner cannot be read. Runs before the first `hello`, so no request
/// reaches a stranger, and a refusal is never retried.
fn ensure_trusted_endpoint(stream: &Stream) -> Result<(), ClientError> {
    match peer_is_this_user(stream) {
        Ok(true) => Ok(()),
        Ok(false) => Err(ClientError::UntrustedEndpoint(
            "it is served by another user".to_owned(),
        )),
        Err(error) => Err(ClientError::UntrustedEndpoint(format!(
            "its owner cannot be read: {error}"
        ))),
    }
}

/// On a client stream the peer is the server, so this reads the server process of the pipe and checks its user.
#[cfg(windows)]
fn peer_is_this_user(stream: &Stream) -> io::Result<bool> {
    let pid = stream
        .peer_creds()?
        .pid()
        .ok_or_else(|| io::Error::other("the pipe names no server process"))?;
    massorca_profile::process_belongs_to_current_user(pid)
}

/// The peer's effective uid must be this process's effective uid.
#[cfg(unix)]
fn peer_is_this_user(stream: &Stream) -> io::Result<bool> {
    let uid = stream
        .peer_creds()?
        .euid()
        .ok_or_else(|| io::Error::other("the socket names no peer user"))?;
    Ok(uid == massorca_profile::current_uid())
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
