//! Pairing refuses a profile others could change before it reaches the server, so the one-time offer is not used
//! up (ADR 0073). Temporary homes only.

use std::net::TcpListener;
use std::time::Duration;

use massorca_client::{Client, ClientError, Hosts};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::devices::Secret;
use massorca_protocol::pairing::Offer;

/// Gives everyone write access to the home, as an older build left a home that inherited a shared folder's access.
fn share(home: &std::path::Path) {
    #[cfg(windows)]
    {
        let granted = std::process::Command::new("icacls")
            .arg(home)
            .args(["/grant", "*S-1-1-0:(OI)(CI)(M)"])
            .output()
            .expect("icacls runs");
        assert!(
            granted.status.success(),
            "{}",
            String::from_utf8_lossy(&granted.stderr)
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o777)).expect("chmod");
    }
}

#[tokio::test]
async fn pairing_into_a_profile_others_can_change_fails_before_any_connection() {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let home = temp.path().join("home");
    massorca_profile::create_home(&home).expect("a home");
    share(&home);
    let profile = Profile::resolve_with(Some(home), Some("default".to_owned()), |_| None)
        .expect("the profile resolves");
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local listener");
    listener.set_nonblocking(true).expect("non-blocking");
    let offer = Offer {
        server: "office".to_owned(),
        addresses: vec![listener.local_addr().expect("its address").to_string()],
        pin: "sha256:not-checked-before-the-refusal".to_owned(),
        secret: Secret::new("placeholder-secret"),
        expires_at_ms: i64::MAX,
        scopes: Vec::new(),
    };
    let peer = PeerInfo {
        name: "massorca-test".to_owned(),
        version: "0.0.0-test".to_owned(),
    };

    let paired = Client::pair(&Hosts::of(&profile), &offer, "office", "laptop", &peer).await;

    match paired {
        Err(ClientError::ProfileRefused(body)) => {
            assert_eq!(body.code, "profile.unsafe_location", "{body:?}");
        }
        Err(other) => panic!("expected a refusal, got {other:?}"),
        Ok(_) => panic!("expected a refusal, got a pairing"),
    }
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        listener.accept().is_err(),
        "nothing connected to the server address"
    );
}
