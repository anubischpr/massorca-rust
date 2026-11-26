//! The bridge against a real service on a fake home folder (SKL-02): a preview writes nothing, a placement copies the skill
//! into the agent folder and the canonical root, the placed list follows it, and a removal deletes only the copies MassOrca
//! placed. The service runs in this test's runtime; the bridge runs on its own thread, as in the app.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_desktop::service::{
    ConnectionState, PlacementAnswer, PlacementCall, PlacementOutcome, ServiceBridge,
    ServiceHandle, SkillsListing, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::methods::skill_placement::PlacementAgent;
use massorca_server::{DiscoveryOptions, RunningService, Service, ServiceConfig};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);

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
    assert!(folded.is_ok(), "timed out waiting for {what}");
}

fn placement_answered(state: &StoreState, request: u64) -> bool {
    state
        .placement
        .as_ref()
        .is_some_and(|held| held.request == request)
}

fn placed_list_answered(state: &StoreState, request: u64) -> bool {
    state
        .managed
        .as_ref()
        .is_some_and(|held| held.request == request)
}

/// The placed skills the store holds, from the newest list answer.
fn placed_names(state: &StoreState) -> Vec<String> {
    match state.managed.as_ref().map(|held| &held.outcome) {
        Some(PlacementOutcome::Managed(skills)) => {
            skills.iter().map(|skill| skill.name.clone()).collect()
        }
        other => panic!("no placed list in the store: {other:?}"),
    }
}

#[tokio::test]
async fn the_bridge_previews_places_and_removes_a_home_skill_on_a_fake_home() {
    let profile_home = temp_home();
    let fake = tempfile::tempdir().expect("a temporary folder");
    let user_home = fake.path().join("user-home");
    let skill = user_home.join(".claude/skills/notes");
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
    let skill_id = match &state.skills[&None].listing {
        SkillsListing::Found(found) => found
            .skills
            .iter()
            .find(|found| found.name == "notes")
            .expect("the notes skill")
            .id
            .clone(),
        other => panic!("the skills are not listed: {other:?}"),
    };

    handle.skill_placement(
        2,
        PlacementCall::Preview {
            skill_id: skill_id.clone(),
            agents: vec![PlacementAgent::Cursor],
        },
    );
    until(&mut updates, &mut state, "the preview", |state| {
        placement_answered(state, 2)
    })
    .await;
    assert!(
        matches!(
            state.placement.as_ref(),
            Some(PlacementAnswer {
                outcome: PlacementOutcome::Preview(_),
                ..
            })
        ),
        "the preview is answered: {:?}",
        state.placement
    );
    assert!(
        !user_home.join(".agents").exists(),
        "a preview writes nothing"
    );

    handle.skill_placement(
        3,
        PlacementCall::Place {
            skill_id,
            agents: vec![PlacementAgent::Cursor],
        },
    );
    until(&mut updates, &mut state, "the placement", |state| {
        placement_answered(state, 3)
    })
    .await;
    until(&mut updates, &mut state, "the placed list", |state| {
        placed_list_answered(state, 3)
    })
    .await;
    assert!(user_home.join(".agents/skills/notes/SKILL.md").is_file());
    assert!(user_home.join(".cursor/skills/notes/SKILL.md").is_file());
    assert_eq!(placed_names(&state), ["notes"]);

    handle.skill_placement(
        4,
        PlacementCall::Unplace {
            name: "notes".to_owned(),
        },
    );
    until(&mut updates, &mut state, "the removal", |state| {
        placement_answered(state, 4)
    })
    .await;
    until(
        &mut updates,
        &mut state,
        "the placed list after removal",
        |state| placed_list_answered(state, 4),
    )
    .await;
    assert!(!user_home.join(".cursor/skills/notes").exists());
    assert!(!user_home.join(".agents/skills/notes").exists());
    assert!(
        skill.join("SKILL.md").is_file(),
        "the skill the user had stays"
    );
    assert!(placed_names(&state).is_empty());

    drop(bridge);
    running.shutdown_handle().shutdown();
    timeout(WAIT, running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}
