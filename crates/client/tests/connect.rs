//! Connecting without a running service: unavailable without autostart, and a start that fails or times out with it.

use std::path::Path;
use std::time::{Duration, Instant};

use massorca_client::{Autostart, Client, ClientError, ConnectOptions};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

fn options() -> ConnectOptions {
    ConnectOptions::new(PeerInfo {
        name: "massorca-test-client".to_owned(),
        version: "0.0.0-test".to_owned(),
    })
}

#[tokio::test]
async fn without_autostart_nobody_listening_is_unavailable() {
    let home = tempfile::tempdir().expect("a temporary home");
    let error = Client::connect(&profile(home.path()), options())
        .await
        .expect_err("nothing listens on a fresh profile");
    assert!(
        matches!(error, ClientError::Unavailable(_)),
        "got {error:?}"
    );
    assert_eq!(error.exit_code(), 4);
}

#[tokio::test]
async fn a_missing_executable_fails_to_start_at_once() {
    let home = tempfile::tempdir().expect("a temporary home");
    let mut options = options();
    options.autostart = Some(Autostart {
        exe: home.path().join("no-such-massorca"),
        args_extra: Vec::new(),
    });
    options.connect_timeout = Duration::from_secs(60);

    let started = Instant::now();
    let error = Client::connect(&profile(home.path()), options)
        .await
        .expect_err("the executable does not exist");
    assert!(
        matches!(error, ClientError::StartFailed(_)),
        "got {error:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "failed after {:?}, not at once",
        started.elapsed()
    );
    assert_eq!(error.exit_code(), 4);
}

#[tokio::test]
async fn autostart_gives_up_once_the_connect_timeout_passes() {
    // The test binary stands in for `massorca`: it rejects the `service run` arguments and never listens.
    let home = tempfile::tempdir().expect("a temporary home");
    let mut options = options();
    options.autostart = Some(Autostart {
        exe: std::env::current_exe().expect("the test binary path"),
        args_extra: Vec::new(),
    });
    options.connect_timeout = Duration::from_secs(1);

    let error = Client::connect(&profile(home.path()), options)
        .await
        .expect_err("the started process never listens");
    assert!(
        matches!(error, ClientError::StartFailed(_)),
        "got {error:?}"
    );
    assert_eq!(error.exit_code(), 4);
}
