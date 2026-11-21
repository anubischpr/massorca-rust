//! A refused handshake: the client reports the refusal and the protocol range the service names.

use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::traits::tokio::Listener as _;
use massorca_client::{Client, ClientError, ConnectOptions};
use massorca_profile::{Profile, ensure_dir, local_socket_name};
use massorca_protocol::{
    ErrorBody, MAX_FRAME_LEN_LOCAL, Message, PeerInfo, ProtocolRange, codes, read_message,
    write_message,
};

#[tokio::test]
async fn a_refused_handshake_reports_the_range_the_service_speaks() {
    let home = tempfile::tempdir().expect("a temporary home");
    let profile = Profile::resolve_with(
        Some(home.path().to_path_buf()),
        Some("default".to_owned()),
        |_| None,
    )
    .expect("the profile resolves");
    // Unix sockets live inside the profile directory, which the service creates before it binds.
    ensure_dir(&profile).expect("the profile directory exists");
    let listener = ListenerOptions::new()
        .name(local_socket_name(&profile).expect("the socket name"))
        .create_tokio()
        .expect("the stand-in listens");

    // A stand-in service that refuses the hello and names the one version it speaks.
    let stand_in = tokio::spawn(async move {
        let mut stream = listener.accept().await.expect("the client connects");
        let hello = read_message(&mut stream, MAX_FRAME_LEN_LOCAL)
            .await
            .expect("the hello arrives");
        assert!(
            matches!(hello, Some(Message::Hello { .. })),
            "the first frame is hello"
        );
        let refusal = Message::Rejected {
            error: ErrorBody::new(
                codes::PROTOCOL_UNSUPPORTED_VERSION,
                "this service speaks protocol 1 only",
            ),
            supported: ProtocolRange { min: 1, max: 1 },
        };
        write_message(&mut stream, &refusal)
            .await
            .expect("the refusal is sent");
    });

    let options = ConnectOptions::new(PeerInfo {
        name: "massorca-test-client".to_owned(),
        version: "0.0.0-test".to_owned(),
    });
    let error = Client::connect(&profile, options)
        .await
        .expect_err("the service refused the handshake");
    stand_in.await.expect("the stand-in finishes");

    match error {
        ClientError::Handshake { error, supported } => {
            assert_eq!(error.code, codes::PROTOCOL_UNSUPPORTED_VERSION);
            assert_eq!(supported, Some(ProtocolRange { min: 1, max: 1 }));
        }
        other => panic!("expected a handshake refusal, got {other:?}"),
    }
}
