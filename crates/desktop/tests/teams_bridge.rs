//! The Bot Teams reads of the bridge against a real service in a temporary profile, driven headlessly: no
//! window opens. The service runs in this test's runtime; the bridge runs on its own thread, as in the app.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::{Client, ConnectOptions};
use massorca_desktop::service::{ServiceBridge, StoreState, StoreUpdate, connect_options};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::tasks::{Column, TaskCreateParams};
use massorca_protocol::methods::teams::TeamCreateParams;
use massorca_server::{RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

/// A private home below a temporary folder; the temporary folder itself may be shared with other users (ADR 0073).
struct TempHome {
    _temp: TempDir,
    path: PathBuf,
}

impl TempHome {
    fn path(&self) -> &Path {
        &self.path
    }
}

fn temp_home() -> TempHome {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let path = temp.path().join("home");
    massorca_profile::create_home(&path).expect("a private home");
    TempHome { _temp: temp, path }
}

const WAIT: Duration = Duration::from_secs(30);

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

async fn start_service(home: &Path) -> RunningService {
    Service::start(ServiceConfig {
        profile: profile(home),
        server_name: "massorca-test".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    })
    .await
    .expect("the service starts")
}

async fn stop_service(running: RunningService) {
    running.shutdown_handle().shutdown();
    timeout(WAIT, running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

/// A client of the same service, as the CLI would be.
async fn client_of(home: &Path) -> Client {
    Client::connect(
        &profile(home),
        ConnectOptions::new(PeerInfo {
            name: "massorca-test-client".to_owned(),
            version: "0.0.0-test".to_owned(),
        }),
    )
    .await
    .expect("a client connects")
}

fn bridge_for(home: &Path) -> (ServiceBridge, UnboundedReceiver<StoreUpdate>) {
    let mut bridge =
        ServiceBridge::start(profile(home), connect_options(None)).expect("the bridge starts");
    let updates = bridge.take_updates().expect("the updates are there");
    (bridge, updates)
}

/// Folds the bridge's updates into a store state until `done` holds.
async fn until(
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
    what: &str,
    done: impl Fn(&StoreState) -> bool,
) {
    let folded = timeout(WAIT, async {
        while !done(state) {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await;
    assert!(folded.is_ok(), "timed out waiting for {what}: {state:?}");
}

#[tokio::test]
async fn the_bridge_reads_every_team_with_its_board() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let client = client_of(home.path()).await;
    let team = client
        .team_create(&TeamCreateParams {
            name: "Desarrollo".to_owned(),
            purpose: Some("Ship the Teams surface".to_owned()),
            character: None,
            access: None,
            workspaces: Some(true),
        })
        .await
        .expect("the team is created");
    client
        .task_create(&TaskCreateParams {
            team: team.id.clone(),
            title: "Write the spec".to_owned(),
            body: None,
            bot: None,
            project: None,
            origin: None,
        })
        .await
        .expect("the task is created");

    let (_bridge, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(
        &mut updates,
        &mut state,
        "the team with its card",
        |state| {
            state
                .teams
                .as_deref()
                .is_some_and(|records| !records.is_empty())
        },
    )
    .await;
    let records = state.teams.as_deref().expect("the teams are read");
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.team.name, "Desarrollo");
    assert_eq!(record.team.purpose, "Ship the Teams surface");
    assert!(record.team.workspaces);
    let open = &record.board.columns[0];
    assert_eq!(open.column, Column::Open);
    let titles: Vec<&str> = open.cards.iter().map(|card| card.title.as_str()).collect();
    assert_eq!(titles, ["Write the spec"]);
    stop_service(service).await;
}

#[tokio::test]
async fn a_team_created_through_another_client_reaches_the_store_without_a_request() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first read", |state| {
        state.teams.as_deref().is_some()
    })
    .await;
    assert!(
        state
            .teams
            .as_deref()
            .is_some_and(|records| records.is_empty())
    );

    let client = client_of(home.path()).await;
    client
        .team_create(&TeamCreateParams {
            name: "Ops".to_owned(),
            purpose: None,
            character: None,
            access: None,
            workspaces: None,
        })
        .await
        .expect("the team is created");
    until(
        &mut updates,
        &mut state,
        "the team created through the service",
        |state| {
            state
                .teams
                .as_deref()
                .is_some_and(|records| records.len() == 1)
        },
    )
    .await;
    stop_service(service).await;
}
