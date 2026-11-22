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
use crate::transport;

/// The first pause between attempts to reach a starting service. It doubles up to `MAX_PAUSE`.
const INITIAL_PAUSE: Duration = Duration::from_millis(25);
const MAX_PAUSE: Duration = Duration::from_millis(250);

pub(crate) async fn connect(
    profile: &Profile,
    options: ConnectOptions,
) -> Result<Client, ClientError> {
    let name = local_socket_name(profile)?;
    let deadline = Instant::now() + options.connect_timeout;
    loop {
        let stream = establish(profile, name.clone(), &options).await?;
        ensure_trusted_endpoint(&stream)?;
        let mut io = transport::spawn_local(stream);
        let exchanged = handshake::exchange(&mut io, &options.client, None).await;
        if exchanged.is_err() {
            io.abort();
        }
        match exchanged {
            Ok(welcome) => return Ok(Client::open(io, welcome)),
            // A service that is letting go of the endpoint may close a connection before welcome. With autostart allowed,
            // the next attempt reaches its successor or starts one.
            Err(ClientError::Disconnected)
                if options.autostart.is_some() && Instant::now() < deadline =>
            {
                sleep(INITIAL_PAUSE).await;
            }
            Err(error) => return Err(error),
        }
    }
}

/// The first connection of a client: reaches the endpoint, or starts the service when nothing listens and autostart is set.
async fn establish(
    profile: &Profile,
    name: Name<'static>,
    options: &ConnectOptions,
) -> Result<Stream, ClientError> {
    match Stream::connect(name.clone()).await {
        Ok(stream) => Ok(stream),
        Err(error) if nobody_listens(&error) => match &options.autostart {
            Some(autostart) => {
                let mut starting = spawn::service_run(profile, autostart)?;
                let reached =
                    wait_for_listener(name, options.connect_timeout, Some(&mut starting)).await;
                starting.release();
                reached
            }
            None => Err(ClientError::Unavailable(error)),
        },
        // The endpoint is still held by a service that is letting go of it, so wait for it as a start would.
        Err(error) if options.autostart.is_some() && is_busy(&error) => {
            wait_for_listener(name, options.connect_timeout, None).await
        }
        Err(error) => Err(ClientError::Unavailable(error)),
    }
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

/// Retries until the started service accepts a connection, or the timeout passes. A service this client started that
/// exits first is reported with the refusal it left, so the user sees why instead of a timeout.
async fn wait_for_listener(
    name: Name<'static>,
    timeout: Duration,
    mut starting: Option<&mut spawn::Starting>,
) -> Result<Stream, ClientError> {
    let deadline = Instant::now() + timeout;
    let mut pause = INITIAL_PAUSE;
    loop {
        match Stream::connect(name.clone()).await {
            Ok(stream) => return Ok(stream),
            Err(error) if nobody_listens(&error) || is_busy(&error) => {}
            Err(error) => return Err(ClientError::Unavailable(error)),
        }
        if let Some(refusal) = starting.as_deref_mut().and_then(spawn::Starting::refusal) {
            return Err(refusal);
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

/// A busy pipe, or one a service has not yet released for its successor (Windows reports access denied for that).
fn is_busy(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::PermissionDenied
    )
}
