//! `massorca host forward NAME PORT` (decision 0131): listens on a local port and carries each connection to a loopback
//! port on the host over the paired connection, until interrupted or stopped on the host. `--list` and `--stop` read and
//! stop the host's forwards.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use massorca_client::{Client, ForwardEnd, ForwardHandle};
use massorca_protocol::capabilities;
use massorca_protocol::methods::forwards::{
    ForwardInfo, ForwardParams, MAX_CONNECTIONS_PER_FORWARD,
};
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use crate::args::HostForwardArgs;
use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::target::Target;
use crate::workspace_text;

/// What `host forward` prints when its forward starts, and again when it ends.
#[derive(Debug, Serialize)]
pub struct ForwardState {
    /// `forwarding`, then `stopped` (interrupted here, or stopped on the host).
    pub state: &'static str,
    pub host: String,
    pub forward: String,
    pub port: u16,
    pub local_address: IpAddr,
    pub local_port: u16,
    /// The local URL to open: the dev server's printed URL with this side's address and port, or a plain `http://`.
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
}

/// Runs `host forward` against `target`, the host NAME names.
pub async fn execute(
    target: &Target,
    args: &HostForwardArgs,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(target, false).await?;
    let host = target.host.clone().unwrap_or_else(|| "local".to_owned());
    if args.list {
        return list(&client, output).await;
    }
    if let Some(forward) = &args.stop {
        let stopped = client.forward_stop(forward).await?;
        let line = format!(
            "stopped forward {} of port {}",
            stopped.forward.id, stopped.forward.port
        );
        output.success(&stopped, &[line]);
        return Ok(());
    }
    let Some(port) = args.port else {
        return Err(CliError::new(
            "cli.usage",
            "host forward needs a PORT",
            exit::USAGE,
        ));
    };
    // Refused before the local port is taken when the host cannot forward.
    for capability in [capabilities::PORT_FORWARD, capabilities::BINARY_FRAMES] {
        if !client.has_capability(capability) {
            return Err(CliError::new(
                "service.capability_missing",
                format!(
                    "host {host} cannot forward ports: its service offers no {capability}; update it there"
                ),
                exit::ERROR,
            ));
        }
    }
    let listener = bind(args.bind, args.local.unwrap_or(port)).await?;
    let local = listener.local_addr().map_err(io_failure)?;
    if !args.bind.is_loopback() {
        output.progress(&format!(
            "warning: the forward listens on {}, so other machines can reach host port {port} through it",
            args.bind
        ));
    }
    let params = ForwardParams {
        port,
        address: None,
        local_port: Some(local.port()),
    };
    let mut forward = client.forward_open(&params).await?;
    let mut state = ForwardState {
        state: "forwarding",
        host: host.clone(),
        forward: forward.info().id.clone(),
        port,
        local_address: local.ip(),
        local_port: local.port(),
        url: local_url(&client, port, local).await,
        reason: None,
    };
    output.success(
        &state,
        &[format!(
            "forwarding {local} to port {port} on {host} (forward {}); open {}; press Ctrl+C to stop",
            state.forward, state.url
        )],
    );
    let end = serve(&client, &listener, &mut forward, output).await;
    drop(listener);
    match end {
        Served::Interrupted => state.reason = Some("interrupted"),
        Served::Ended(ForwardEnd::Stopped(_)) => state.reason = Some("stopped on the host"),
        Served::Ended(ForwardEnd::Lost) => {
            return Err(CliError::new(
                "host.unreachable",
                format!("the connection to {host} was lost, so the forward ended"),
                exit::UNAVAILABLE,
            ));
        }
        Served::Ended(ForwardEnd::Ended(error)) => {
            return Err(error.map_or_else(
                || CliError::new("forward.ended", "the host ended the forward", exit::ERROR),
                CliError::from,
            ));
        }
    }
    state.state = "stopped";
    let line = format!(
        "stopped forward {} ({})",
        state.forward,
        state.reason.unwrap_or_default()
    );
    output.success(&state, &[line]);
    Ok(())
}

enum Served {
    Interrupted,
    Ended(ForwardEnd),
}

/// Accepts local connections until interrupted or the forward ends. Past the per-forward limit a connection is closed at
/// once.
async fn serve(
    client: &Client,
    listener: &TcpListener,
    forward: &mut ForwardHandle,
    output: Output,
) -> Served {
    let id = forward.info().id.clone();
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS_PER_FORWARD));
    let interrupted = tokio::signal::ctrl_c();
    tokio::pin!(interrupted);
    loop {
        tokio::select! {
            _ = &mut interrupted => return Served::Interrupted,
            end = forward.ended() => return Served::Ended(end),
            accepted = listener.accept() => {
                let Ok((socket, _peer)) = accepted else {
                    continue;
                };
                let Ok(slot) = Arc::clone(&slots).try_acquire_owned() else {
                    output.progress("refused a connection: the forward carries the most connections it may");
                    continue;
                };
                let _ = socket.set_nodelay(true);
                let client = client.clone();
                let id = id.clone();
                tokio::spawn(async move {
                    let _slot = slot;
                    if let Err(error) = client.forward_tunnel(&id, socket).await {
                        output.progress(&format!("a connection was not forwarded: {error}"));
                    }
                });
            }
        }
    }
}

async fn bind(address: IpAddr, port: u16) -> Result<TcpListener, CliError> {
    TcpListener::bind(SocketAddr::new(address, port))
        .await
        .map_err(|error| {
            let mut failure = CliError::new(
                "forward.local_port_busy",
                format!("cannot listen on {address}:{port}: {error}; pass --local PORT, or --local 0 for a free one"),
                exit::ERROR,
            );
            if error.kind() != std::io::ErrorKind::AddrInUse {
                failure.code = "forward.local_bind_failed".to_owned();
            }
            failure
        })
}

/// The dev server's printed URL for `port` with this side's address, when `host ports` knows one; else `http://`.
async fn local_url(client: &Client, port: u16, local: SocketAddr) -> String {
    let printed = if client.has_capability(capabilities::HOST) {
        client.host_ports().await.ok().and_then(|ports| {
            ports
                .ports
                .into_iter()
                .find(|entry| entry.port == port && entry.url_printed)
                .and_then(|entry| entry.url)
        })
    } else {
        None
    };
    rebase(printed.as_deref(), local)
}

/// `url` with its host and port replaced by `local`, keeping its scheme and path.
fn rebase(url: Option<&str>, local: SocketAddr) -> String {
    let (scheme, rest) = url
        .and_then(|url| url.split_once("://"))
        .unwrap_or(("http", "/"));
    let path = rest
        .find(['/', '?', '#'])
        .map_or("/", |start| &rest[start..]);
    format!("{scheme}://{local}{path}")
}

async fn list(client: &Client, output: Output) -> Result<(), CliError> {
    let listed = client.forward_list().await?;
    let lines = if listed.forwards.is_empty() {
        vec!["no forwards".to_owned()]
    } else {
        workspace_text::table(
            &[
                "FORWARD",
                "PORT",
                "LOCAL",
                "DEVICE",
                "OPEN",
                "TOTAL",
                "TO HOST",
                "FROM HOST",
            ],
            listed.forwards.iter().map(row).collect(),
        )
    };
    output.success(&listed, &lines);
    Ok(())
}

fn row(forward: &ForwardInfo) -> Vec<String> {
    vec![
        forward.id.clone(),
        forward.port.to_string(),
        forward
            .local_port
            .map_or_else(|| "-".to_owned(), |port| port.to_string()),
        forward
            .device_id
            .clone()
            .unwrap_or_else(|| "owner".to_owned()),
        forward.connections.to_string(),
        forward.total_connections.to_string(),
        forward.bytes_to_host.to_string(),
        forward.bytes_from_host.to_string(),
    ]
}

fn io_failure(error: std::io::Error) -> CliError {
    CliError::new("forward.local_bind_failed", error.to_string(), exit::ERROR)
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};

    use super::rebase;

    #[test]
    fn a_printed_url_keeps_its_scheme_and_path_on_the_local_port() {
        let local = SocketAddr::from((Ipv4Addr::LOCALHOST, 8080));
        assert_eq!(
            rebase(Some("https://localhost:5173/app/?x=1"), local),
            "https://127.0.0.1:8080/app/?x=1"
        );
        assert_eq!(
            rebase(Some("http://[::1]:3000"), local),
            "http://127.0.0.1:8080/"
        );
        assert_eq!(rebase(None, local), "http://127.0.0.1:8080/");
    }
}
