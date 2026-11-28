//! Port forwards (capability `port-forward/1`, decision 0131), against the real service: echo servers on loopback stand
//! for the host's dev servers, and an in-memory pipe stands for each local connection.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use massorca_client::{Client, ClientError, ForwardEnd, SpliceEnd};
use massorca_protocol::codes;
use massorca_protocol::methods::forwards::ForwardParams;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream, duplex};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::common::{connect, remote_code, start, stop, temp_home};
use crate::network::{client_side, offer, pair, start_listening};

const WAIT: Duration = Duration::from_secs(30);

/// An echo server on loopback: each connection gets back what it sends, and is closed once the client ends its data.
async fn echo_server() -> u16 {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("an echo listener");
    let port = listener.local_addr().expect("its address").port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let (mut read, mut write) = socket.split();
                let _ = tokio::io::copy(&mut read, &mut write).await;
                let _ = write.shutdown().await;
            });
        }
    });
    port
}

fn params(port: u16) -> ForwardParams {
    ForwardParams {
        port,
        address: None,
        local_port: Some(1),
    }
}

/// Starts one tunnelled connection and returns the local end of it.
fn tunnel(
    client: &Client,
    forward: &str,
) -> (DuplexStream, JoinHandle<Result<SpliceEnd, ClientError>>) {
    let (local, socket) = duplex(64 * 1024);
    let client = client.clone();
    let forward = forward.to_owned();
    let task = tokio::spawn(async move {
        client
            .forward_tunnel(&forward, socket)
            .await
            .map(|report| report.end)
    });
    (local, task)
}

/// Sends `message` through a new tunnel, ends its data, and returns what came back before the tunnel closed.
async fn round_trip(client: &Client, forward: &str, message: &[u8]) -> Vec<u8> {
    let (local, task) = tunnel(client, forward);
    let (mut read, mut write) = tokio::io::split(local);
    let sending = async {
        write.write_all(message).await.expect("send");
        write.shutdown().await.expect("end of data");
    };
    let mut back = Vec::new();
    let receiving = read.read_to_end(&mut back);
    let ((), received) = timeout(WAIT, async { tokio::join!(sending, receiving) })
        .await
        .expect("the round trip finishes");
    received.expect("read the echo");
    let end = timeout(WAIT, task)
        .await
        .expect("the tunnel ends")
        .expect("the tunnel task")
        .expect("the tunnel opens");
    assert!(matches!(end, SpliceEnd::Closed), "{end:?}");
    back
}

#[tokio::test(flavor = "multi_thread")]
async fn a_forward_carries_bytes_both_ways_for_sequential_and_concurrent_connections() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let port = echo_server().await;
    let forward = client
        .forward_open(&params(port))
        .await
        .expect("host.forward");
    let id = forward.info().id.clone();
    assert_eq!(forward.info().port, port);

    for round in 0..25 {
        let message = format!("round {round}").into_bytes();
        assert_eq!(round_trip(&client, &id, &message).await, message);
    }

    // Larger than the window both ways, five at once.
    let mut concurrent = Vec::new();
    for lane in 0u8..5 {
        let client = client.clone();
        let id = id.clone();
        concurrent.push(tokio::spawn(async move {
            let message: Vec<u8> = (0..600 * 1024).map(|at| (at % 251) as u8 ^ lane).collect();
            let back = round_trip(&client, &id, &message).await;
            assert!(back == message, "lane {lane} came back changed");
        }));
    }
    for lane in concurrent {
        lane.await.expect("a concurrent connection");
    }

    let listed = client.forward_list().await.expect("host.forwards");
    let entry = listed
        .forwards
        .iter()
        .find(|entry| entry.id == id)
        .expect("the forward is listed");
    assert_eq!(entry.total_connections, 30, "{entry:?}");
    assert_eq!(entry.connections, 0, "{entry:?}");
    assert!(entry.bytes_to_host >= 5 * 600 * 1024, "{entry:?}");
    assert_eq!(entry.bytes_to_host, entry.bytes_from_host, "{entry:?}");

    drop(forward);
    let deadline = tokio::time::Instant::now() + WAIT;
    while !client
        .forward_list()
        .await
        .expect("host.forwards")
        .forwards
        .is_empty()
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "a dropped forward stays open"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn closing_either_side_closes_the_other() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    // The host side says goodbye and closes: the local side reads it, then end of data.
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind");
    let port = listener.local_addr().expect("address").port();
    let host_side = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        socket.write_all(b"bye").await.expect("write");
        drop(socket);
        let (mut socket, _) = listener.accept().await.expect("accept again");
        let mut rest = Vec::new();
        let read = timeout(WAIT, socket.read_to_end(&mut rest)).await;
        (rest, read.is_ok())
    });
    let forward = client
        .forward_open(&params(port))
        .await
        .expect("host.forward");
    let id = forward.info().id.clone();

    let (mut local, task) = tunnel(&client, &id);
    let mut got = Vec::new();
    timeout(WAIT, local.read_to_end(&mut got))
        .await
        .expect("the local side sees the close")
        .expect("read");
    assert_eq!(got, b"bye");
    drop(local);
    let end = timeout(WAIT, task)
        .await
        .expect("the tunnel ends")
        .expect("task");
    assert!(end.is_ok(), "{end:?}");

    // The local side writes and goes away: the host side reads the bytes, then end of data.
    let (mut local, task) = tunnel(&client, &id);
    local.write_all(b"last words").await.expect("write");
    drop(local);
    let (rest, ended) = host_side.await.expect("the host side");
    assert!(ended, "the host side never saw the local side close");
    assert_eq!(rest, b"last words");
    let _ = timeout(WAIT, task).await.expect("the tunnel ends");
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_targets_are_refused_and_stop_ends_the_forward() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let outside = ForwardParams {
        address: Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))),
        ..params(80)
    };
    let refused = client
        .forward_open(&outside)
        .await
        .expect_err("not loopback");
    assert_eq!(remote_code(&refused), codes::FORWARD_TARGET_NOT_LOOPBACK);

    // Nothing listens on a port just released: the connection is refused, the forward stays.
    let free = std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .expect("bind")
        .local_addr()
        .expect("address")
        .port();
    let mut forward = client
        .forward_open(&params(free))
        .await
        .expect("host.forward");
    let id = forward.info().id.clone();
    let (_local, socket) = duplex(1024);
    let failed = client
        .forward_tunnel(&id, socket)
        .await
        .expect_err("nothing listens");
    assert_eq!(remote_code(&failed), codes::FORWARD_CONNECT_FAILED);
    let unknown = client
        .forward_tunnel("f-999", duplex(16).1)
        .await
        .expect_err("unknown");
    assert_eq!(remote_code(&unknown), codes::FORWARD_NOT_FOUND);

    let other = connect(home.path(), "default").await;
    let foreign = other
        .forward_tunnel(&id, duplex(16).1)
        .await
        .expect_err("another connection's forward");
    assert_eq!(remote_code(&foreign), codes::FORWARD_NOT_FOUND);
    // Stopping reaches every caller's forwards, so an unknown id is not said to be someone else's.
    let unknown = other
        .forward_stop("f-999")
        .await
        .expect_err("no such forward");
    assert_eq!(remote_code(&unknown), codes::FORWARD_NOT_FOUND);
    let ClientError::Remote(body) = &unknown else {
        panic!("expected a refusal, got {unknown:?}");
    };
    assert_eq!(body.message, "no open forward is named f-999", "{body:?}");
    let stopped = other.forward_stop(&id).await.expect("host.forward_stop");
    assert_eq!(stopped.forward.id, id);
    match timeout(WAIT, forward.ended())
        .await
        .expect("the forward ends")
    {
        ForwardEnd::Stopped(info) => assert_eq!(info.id, id),
        other => panic!("expected a stop, got {other:?}"),
    }
    assert!(
        client
            .forward_list()
            .await
            .expect("host.forwards")
            .forwards
            .is_empty()
    );
    stop(running).await;
}

/// A forward reaches the host's loopback ports, so it needs `host-admin`: a standard device (`read` and `workspace`) is
/// refused, though it may list forwards.
#[tokio::test(flavor = "multi_thread")]
async fn a_device_without_host_admin_cannot_open_connect_or_stop_a_forward() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let (standard, _) = pair(&hosts, &offer(home.path(), false, None).await, "plain")
        .await
        .expect("a standard device pairs");
    let (admin, _) = pair(&hosts, &offer(home.path(), true, None).await, "admin")
        .await
        .expect("an admin device pairs");
    let port = echo_server().await;

    let denied = standard
        .forward_open(&params(port))
        .await
        .expect_err("no host-admin");
    assert_eq!(remote_code(&denied), codes::PERMISSION_DENIED);

    let forward = admin
        .forward_open(&params(port))
        .await
        .expect("an admin forwards");
    let id = forward.info().id.clone();
    assert!(forward.info().device_id.is_some(), "{:?}", forward.info());
    let message = b"over the network".to_vec();
    assert_eq!(round_trip(&admin, &id, &message).await, message);

    let listed = standard
        .forward_list()
        .await
        .expect("listing needs read only");
    assert_eq!(listed.forwards.len(), 1, "{listed:?}");
    let denied = standard.forward_stop(&id).await.expect_err("no host-admin");
    assert_eq!(remote_code(&denied), codes::PERMISSION_DENIED);
    let denied = standard
        .forward_tunnel(&id, duplex(16).1)
        .await
        .expect_err("no host-admin");
    assert_eq!(remote_code(&denied), codes::PERMISSION_DENIED);
    drop(forward);
    stop(running).await;
}
