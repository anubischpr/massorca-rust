//! Client failures map to the exit codes of the CLI contract: 1 for service and local errors, 4 for an unreachable service.

use std::io;

use massorca_client::ClientError;
use massorca_protocol::{ErrorBody, ProtocolRange, codes};

#[test]
fn service_and_local_errors_exit_with_one() {
    assert_eq!(
        ClientError::Remote(ErrorBody::new(codes::METHOD_NOT_FOUND, "no such method")).exit_code(),
        1
    );
    assert_eq!(
        ClientError::Protocol("a bad frame".to_owned()).exit_code(),
        1
    );
    assert_eq!(
        ClientError::Io(io::Error::other("disk full")).exit_code(),
        1
    );
}

#[test]
fn an_unreachable_service_exits_with_four() {
    assert_eq!(
        ClientError::Unavailable(io::Error::from(io::ErrorKind::NotFound)).exit_code(),
        4
    );
    assert_eq!(
        ClientError::StartFailed("no executable".to_owned()).exit_code(),
        4
    );
    assert_eq!(ClientError::Disconnected.exit_code(), 4);
    assert_eq!(
        ClientError::Handshake {
            error: Box::new(ErrorBody::new(
                codes::PROTOCOL_UNSUPPORTED_VERSION,
                "no shared version",
            )),
            supported: Some(ProtocolRange { min: 1, max: 1 }),
        }
        .exit_code(),
        4
    );
}

#[test]
fn remote_errors_show_their_stable_code() {
    let error = ClientError::Remote(ErrorBody::new(
        codes::CONFIG_UNKNOWN_KEY,
        "no setting named x",
    ));
    assert!(error.to_string().contains("config.unknown_key"), "{error}");
}

#[test]
fn an_untrusted_endpoint_exits_with_four_and_names_its_code() {
    let error = ClientError::UntrustedEndpoint("it is served by another user".to_owned());
    assert_eq!(error.exit_code(), 4);
    assert!(
        error.to_string().contains("service.untrusted_endpoint"),
        "{error}"
    );
}
