//! A host whose service predates port forwards (decision 0131): it offers no `port-forward/1`, so every forward call is
//! refused on the client with `CapabilityMissing`, and no forward request or stream bytes ever reach the host, which would
//! drop the bytes silently.

use std::path::Path;
use std::time::Duration;

use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::traits::tokio::Listener as _;
use massorca_client::{Client, ClientError, ConnectOptions};
use massorca_profile::{Profile, ensure_dir, local_socket_name};
use massorca_protocol::methods::forwards::ForwardParams;
use massorca_protocol::{
    Frame, MAX_FRAME_LEN_LOCAL, Message, PROTOCOL_VERSION, PeerInfo, capabilities, read_frame,
    write_frame,
};
use serde_json::json;
use tokio::task::JoinHandle;
use tokio::time::timeout;

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

/// A service from before `port-forward/1`. It answers every request and returns the methods it was asked for, and
/// whether any stream bytes arrived, once the client leaves.
async fn old_host(home: &Path) -> (Profile, JoinHandle<(Vec<String>, bool)>) {
    let profile = profile(home);
    ensure_dir(&profile).expect("the profile directory exists");
    let listener = ListenerOptions::new()
        .name(local_socket_name(&profile).expect("the socket name"))
        .create_tokio()
        .expect("the stand-in listens");
    let task = tokio::spawn(async move {
        let mut stream = listener.accept().await.expect("the client connects");
        let _hello = read_frame(&mut stream, MAX_FRAME_LEN_LOCAL)
            .await
            .expect("the hello arrives");
        let welcome = Message::Welcome {
            protocol: PROTOCOL_VERSION,
            server: PeerInfo {
                name: "old-host".to_owned(),
                version: "0.0.0-test".to_owned(),
            },
            capabilities: vec![
                capabilities::SERVICE.to_owned(),
                capabilities::HOST.to_owned(),
                capabilities::BINARY_FRAMES.to_owned(),
            ],
            connection_id: "connection-1".to_owned(),
            max_frame_len: None,
        };
        write_frame(&mut stream, &Frame::Message(welcome), MAX_FRAME_LEN_LOCAL)
            .await
            .expect("the welcome is sent");
        let mut methods = Vec::new();
        let mut bytes = false;
        while let Ok(Some(frame)) = read_frame(&mut stream, MAX_FRAME_LEN_LOCAL).await {
            match frame {
                Frame::Message(Message::Request { id, method, .. }) => {
                    methods.push(method);
                    let answer = Message::response_ok(id, json!({}));
                    let _ = write_frame(&mut stream, &Frame::Message(answer), MAX_FRAME_LEN_LOCAL)
                        .await;
                }
                Frame::StreamBytes { .. } => bytes = true,
                Frame::Message(_) => {}
            }
        }
        (methods, bytes)
    });
    (profile, task)
}

fn missing(error: ClientError) -> String {
    match error {
        ClientError::CapabilityMissing { capability, .. } => capability,
        other => panic!("expected a missing capability, got {other:?}"),
    }
}

#[tokio::test]
async fn a_host_without_port_forward_refuses_every_forward_call_before_sending() {
    let home = tempfile::tempdir().expect("a temp home");
    let (profile, host) = old_host(home.path()).await;
    let client = Client::connect(
        &profile,
        ConnectOptions::new(PeerInfo {
            name: "massorca-test-client".to_owned(),
            version: "0.0.0-test".to_owned(),
        }),
    )
    .await
    .expect("the client connects");

    let params = ForwardParams {
        port: 3000,
        address: None,
        local_port: None,
    };
    let refused = client.forward_open(&params).await.expect_err("no forwards");
    assert_eq!(missing(refused), capabilities::PORT_FORWARD);
    let (_local, socket) = tokio::io::duplex(16);
    let refused = client
        .forward_tunnel("f-1", socket)
        .await
        .expect_err("no forwards");
    assert_eq!(missing(refused), capabilities::PORT_FORWARD);
    assert_eq!(
        missing(client.forward_list().await.expect_err("no forwards")),
        capabilities::PORT_FORWARD
    );
    assert_eq!(
        missing(client.forward_stop("f-1").await.expect_err("no forwards")),
        capabilities::PORT_FORWARD
    );
    // The connection still works, and the host saw only this request.
    let _: serde_json::Value = client
        .call("service.info", &())
        .await
        .expect("service.info");
    drop(client);
    let (methods, bytes) = timeout(Duration::from_secs(10), host)
        .await
        .expect("the stand-in ends")
        .expect("the stand-in task");
    assert_eq!(methods, ["service.info"]);
    assert!(!bytes, "stream bytes reached a host that would drop them");
}
