//! This computer's Git, read from a real service on connect (GIT-01). The bridge runs on its own
//! thread, as in the app, and no window opens. The machine running the test has Git, so the
//! service reports it present with a version, and the Settings row would show Installed.

use std::path::Path;
use std::time::{Duration, Instant};

use massorca_desktop::model::git_availability::{GitState, git_row};
use massorca_desktop::service::{ServiceBridge, StoreState, connect_options};
use massorca_profile::Profile;
use massorca_protocol::methods::service::GitStatus;
use massorca_server::{Service, ServiceConfig};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

#[tokio::test]
async fn a_connect_reads_this_computers_git_from_the_real_service() {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let home = temp.path().join("home");
    massorca_profile::create_home(&home).expect("a private home");
    let running = Service::start(ServiceConfig {
        profile: profile(&home),
        server_name: "massorca-test".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    })
    .await
    .expect("the service starts");

    let mut bridge =
        ServiceBridge::start(profile(&home), connect_options(None)).expect("the bridge starts");
    let mut updates = bridge.take_updates().expect("the updates are there");
    let mut state = StoreState::default();
    let answered = timeout(WAIT, async {
        while state.service_git.is_none() {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await;
    assert!(
        answered.is_ok(),
        "timed out waiting for the Git answer: {state:?}"
    );

    let Some(Ok(Some(GitStatus::Present { version }))) = state.service_git.as_ref() else {
        panic!(
            "Git is on this machine, so the service reports it present: {:?}",
            state.service_git
        );
    };
    let major = version
        .split('.')
        .next()
        .and_then(|part| part.parse::<u32>().ok());
    assert!(
        major.is_some_and(|major| major >= 2),
        "a version at or above the 2.25 baseline: {version}"
    );
    let row = git_row(state.service_git.as_ref());
    assert_eq!(
        row.state,
        GitState::Present {
            version: version.clone()
        }
    );
    assert_eq!(row.word, "Installed");

    drop(bridge);
    running.shutdown_handle().shutdown();
    timeout(WAIT, running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}
