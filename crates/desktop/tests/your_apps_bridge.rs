//! Your apps through the bridge against a real service on a fake home folder (SKL-27): a skill an agent tool folder holds is
//! an installed app with no menu, a placement makes it a placed app whose menu offers Remove placement, and the removal
//! takes that offer away again. The service starts with discovery pointed at the fake home and no vendor command runs;
//! the service's vault is not opened, so no token is involved. The bridge runs on its own thread, as in the app.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_desktop::model::marketplace::{
    AppAction, AppRow, managed_rows, skill_apps, skill_rows,
};
use massorca_desktop::service::{
    ConnectionState, PlacementCall, PlacementOutcome, ServiceBridge, ServiceHandle, SkillsListing,
    StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::methods::skill_placement::PlacementAgent;
use massorca_server::{DiscoveryOptions, RunningService, Service, ServiceConfig};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);
const SKILL: &str = "notes";
const KEY: &str = "skill:notes";

/// A private home below a temporary folder; the temporary folder itself may be shared with other users (ADR 0073).
struct TempHome {
    _temp: tempfile::TempDir,
    path: PathBuf,
}

fn temp_home() -> TempHome {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let path = temp.path().join("home");
    massorca_profile::create_home(&path).expect("a private home");
    TempHome { _temp: temp, path }
}

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

fn bridge_for(home: &Path) -> (ServiceBridge, ServiceHandle, UnboundedReceiver<StoreUpdate>) {
    let mut bridge =
        ServiceBridge::start(profile(home), connect_options(None)).expect("the bridge starts");
    let updates = bridge.take_updates().expect("the updates are there");
    let handle = bridge.handle();
    (bridge, handle, updates)
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

fn placed_list_answered(state: &StoreState, request: u64) -> bool {
    state
        .managed
        .as_ref()
        .is_some_and(|held| held.request == request)
}

/// The Your apps rows of the skills the store holds: the discovered skills and the placed list, as the Installed list
/// builds them for This computer.
fn apps_of(state: &StoreState) -> Vec<AppRow> {
    let found = match state.skills.get(&None).map(|read| &read.listing) {
        Some(SkillsListing::Found(discovery)) => skill_rows(discovery),
        other => panic!("the skills are not listed: {other:?}"),
    };
    let placed = match state.managed.as_ref().map(|held| &held.outcome) {
        Some(PlacementOutcome::Managed(skills)) => managed_rows(skills),
        other => panic!("no placed list in the store: {other:?}"),
    };
    skill_apps(&found, &placed, true)
}

fn only_app(state: &StoreState) -> AppRow {
    let rows = apps_of(state);
    assert_eq!(rows.len(), 1, "one skill app: {rows:?}");
    rows.into_iter().next().expect("one row")
}

#[tokio::test]
async fn your_apps_follow_the_installed_and_placed_skill_on_a_fake_home() {
    let profile_home = temp_home();
    let fake = tempfile::tempdir().expect("a temporary folder");
    let user_home = fake.path().join("user-home");
    let skill = user_home.join(".claude/skills").join(SKILL);
    fs::create_dir_all(&skill).expect("the skill folder");
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: notes\ndescription: Notes.\n---\n",
    )
    .expect("the skill file");
    let running: RunningService = Service::start_with_discovery(
        ServiceConfig {
            profile: profile(profile_home.path.as_path()),
            server_name: "massorca-test".to_owned(),
            server_version: "0.0.0-test".to_owned(),
        },
        DiscoveryOptions {
            home: Some(user_home.clone()),
            ..DiscoveryOptions::default()
        },
    )
    .await
    .expect("the service starts");
    let (bridge, handle, mut updates) = bridge_for(profile_home.path.as_path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the connection", |state| {
        matches!(state.connection, ConnectionState::Connected { .. })
    })
    .await;

    handle.read_skills(1, false);
    until(&mut updates, &mut state, "the skill list", |state| {
        state
            .skills
            .get(&None)
            .is_some_and(|read| read.request == 1)
    })
    .await;
    handle.skill_placement(2, PlacementCall::Managed);
    until(&mut updates, &mut state, "the placed list", |state| {
        placed_list_answered(state, 2)
    })
    .await;

    let installed = only_app(&state);
    assert_eq!(
        (installed.key.as_str(), installed.status, installed.action),
        (KEY, "Installed", None),
        "a home skill in an agent folder is installed and has no menu"
    );

    let skill_id = match &state.skills[&None].listing {
        SkillsListing::Found(found) => found
            .skills
            .iter()
            .find(|found| found.name == SKILL)
            .expect("the notes skill")
            .id
            .clone(),
        other => panic!("the skills are not listed: {other:?}"),
    };
    handle.skill_placement(
        3,
        PlacementCall::Place {
            skill_id,
            agents: vec![PlacementAgent::Cursor],
        },
    );
    until(
        &mut updates,
        &mut state,
        "the placed list after placing",
        |state| placed_list_answered(state, 3),
    )
    .await;

    let placed = only_app(&state);
    assert_eq!(placed.status, "Placed");
    assert_eq!(
        placed.action,
        Some(AppAction::RemovePlacement(SKILL.to_owned())),
        "a placed skill offers Remove placement and nothing else"
    );

    handle.skill_placement(
        4,
        PlacementCall::Unplace {
            name: SKILL.to_owned(),
        },
    );
    until(
        &mut updates,
        &mut state,
        "the placed list after removing",
        |state| placed_list_answered(state, 4),
    )
    .await;
    let removed = only_app(&state);
    assert_eq!(
        (removed.status, removed.action),
        ("Installed", None),
        "after the removal the home skill is an installed app again, with no menu"
    );

    drop(bridge);
    running.shutdown_handle().shutdown();
    timeout(WAIT, running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}
