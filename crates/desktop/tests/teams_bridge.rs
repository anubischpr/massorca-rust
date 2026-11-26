//! The Bot Teams reads of the bridge against a real service in a temporary profile, driven headlessly: no
//! window opens. The service runs in this test's runtime; the bridge runs on its own thread, as in the app.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::{Client, ConnectOptions};
use massorca_desktop::model::service_view::teams_view;
use massorca_desktop::model::shell::Agent;
use massorca_desktop::model::teams::Column as PageColumn;
use massorca_desktop::service::{
    Refusal, ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::tasks::{Column, TaskCreateParams};
use massorca_protocol::methods::teams::{
    BoardParams, BotAddParams, BotRemoveParams, BotUpdateParams, CodeSource, TeamAccess,
    TeamCreateParams, TeamProjectAddParams, TeamRef, TeamUpdateParams,
};
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
/// Noticing a stopped service waits on the link's own loss detection, which a loaded runner slows down.
const LINK_LOSS_WAIT: Duration = Duration::from_secs(120);

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
        ConnectOptions::new(PeerInfo { ..base_peer_info() }),
    )
    .await
    .expect("a client connects")
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
    until_within(WAIT, updates, state, what, done).await;
}

/// Folds the bridge's updates until it reports the stopped service: the link is no longer connected.
async fn until_the_link_stops(
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
) {
    until_within(
        LINK_LOSS_WAIT,
        updates,
        state,
        "the link to stop",
        |state| !state.connection.is_connected(),
    )
    .await;
}

async fn until_within(
    budget: Duration,
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
    what: &str,
    done: impl Fn(&StoreState) -> bool,
) {
    let folded = timeout(budget, async {
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
            workspaces: Some(true),
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    client
        .task_create(&TaskCreateParams {
            team: team.id.clone(),
            title: "Write the spec".to_owned(),
            ..base_task_create_params()
        })
        .await
        .expect("the task is created");

    let (_bridge, _handle, mut updates) = bridge_for(home.path());
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
    let view = teams_view(&state);
    assert!(view.is_connected());
    assert_eq!(view.hosts()[0].name, "This PC");
    let team_view = &view.teams()[0];
    assert_eq!(team_view.name, "Desarrollo");
    assert_eq!(team_view.tasks[0].title, "Write the spec");
    assert_eq!(team_view.tasks[0].column, PageColumn::Open);
    stop_service(service).await;
}

#[tokio::test]
async fn a_team_created_through_another_client_reaches_the_store_without_a_request() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, _handle, mut updates) = bridge_for(home.path());
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
            ..base_team_create_params()
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

#[tokio::test]
async fn the_teams_are_not_claimed_once_the_service_is_gone() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, _handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first read", |state| {
        state.teams.is_some()
    })
    .await;
    assert!(teams_view(&state).is_connected());

    stop_service(service).await;
    until_the_link_stops(&mut updates, &mut state).await;
    assert!(!teams_view(&state).is_connected());
}

#[tokio::test]
async fn a_team_and_a_bot_are_written_through_the_bridge_and_read_back() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first read", |state| {
        state.teams.is_some()
    })
    .await;

    handle
        .create_team(TeamCreateParams {
            purpose: Some("Run it".to_owned()),
            workspaces: Some(false),
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    until(
        &mut updates,
        &mut state,
        "the new team to be read",
        |state| {
            teams_view(state)
                .teams()
                .iter()
                .any(|team| team.name == "Ops")
        },
    )
    .await;
    let team_key = {
        let view = teams_view(&state);
        let team = view
            .teams()
            .iter()
            .find(|team| team.name == "Ops")
            .expect("the team is listed");
        state
            .team_ids
            .key(team.id.0)
            .expect("the team is numbered")
            .to_owned()
    };

    handle
        .add_bot(BotAddParams {
            team: team_key,
            ..base_bot_add_params()
        })
        .await
        .expect("the bot is added");
    until(&mut updates, &mut state, "the bot to be listed", |state| {
        teams_view(state)
            .teams()
            .iter()
            .any(|team| team.bots.iter().any(|bot| bot.name == "Nora"))
    })
    .await;
    let view = teams_view(&state);
    let team = view
        .teams()
        .iter()
        .find(|team| team.name == "Ops")
        .expect("the team is listed");
    let bot = &team.bots[0];
    assert_eq!(
        (bot.role.as_str(), bot.agent),
        ("Reviews", Some(Agent::Claude))
    );
    stop_service(service).await;
}

#[tokio::test]
async fn a_name_the_service_refuses_comes_back_as_a_name_problem() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first read", |state| {
        state.teams.is_some()
    })
    .await;
    let desarrollo = || TeamCreateParams {
        name: "Desarrollo".to_owned(),
        ..base_team_create_params()
    };
    handle
        .create_team(desarrollo())
        .await
        .expect("the first team is created");
    let refusal = handle
        .create_team(desarrollo())
        .await
        .expect_err("the same name is refused");
    assert!(
        matches!(&refusal, Refusal::NameProblem { message } if message.contains("another team")),
        "{refusal:?}"
    );
    stop_service(service).await;
}

#[tokio::test]
async fn a_write_is_refused_while_the_service_is_down() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first read", |state| {
        state.teams.is_some()
    })
    .await;
    stop_service(service).await;
    until_the_link_stops(&mut updates, &mut state).await;
    let refusal = handle
        .create_team(TeamCreateParams {
            ..base_team_create_params()
        })
        .await
        .expect_err("nothing is sent while the service is down");
    assert!(
        matches!(refusal, Refusal::Unavailable { .. }),
        "{refusal:?}"
    );
}

#[tokio::test]
async fn a_card_is_finished_and_reopened_through_the_service() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let client = client_of(home.path()).await;
    let team = client
        .team_create(&TeamCreateParams {
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    client
        .task_create(&TaskCreateParams {
            team: team.id.clone(),
            ..base_task_create_params()
        })
        .await
        .expect("the task is created");

    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the card", |state| {
        teams_view(state)
            .teams()
            .iter()
            .any(|team| !team.tasks.is_empty())
    })
    .await;
    let task_key = {
        let view = teams_view(&state);
        let task = view.teams()[0].tasks[0].id;
        state
            .team_ids
            .key(task.0)
            .expect("the card is numbered")
            .to_owned()
    };

    handle
        .finish_task(task_key.clone())
        .await
        .expect("the task is finished");
    until(&mut updates, &mut state, "the card in Done", |state| {
        teams_view(state).teams().iter().any(|team| {
            team.tasks
                .iter()
                .any(|task| task.column == PageColumn::Done)
        })
    })
    .await;

    handle
        .reopen_task(task_key)
        .await
        .expect("the task is reopened");
    until(&mut updates, &mut state, "the card back in Open", |state| {
        teams_view(state).teams().iter().any(|team| {
            team.tasks
                .iter()
                .any(|task| task.column == PageColumn::Open)
        })
    })
    .await;
    stop_service(service).await;
}

/// Params of a bot named Nora, who reviews and runs as Claude.
fn nora(team: &str, lead: bool) -> BotAddParams {
    BotAddParams {
        team: team.to_owned(),
        lead,
        ..base_bot_add_params()
    }
}

#[tokio::test]
async fn a_bot_with_no_card_is_listed_from_the_first_read() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let client = client_of(home.path()).await;
    let team = client
        .team_create(&TeamCreateParams {
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    client
        .team_bot_add(&nora(&team.id, true))
        .await
        .expect("the bot is added");

    let (_bridge, _handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the bot to be listed", |state| {
        teams_view(state)
            .teams()
            .iter()
            .any(|team| team.bots.iter().any(|bot| bot.name == "Nora"))
    })
    .await;
    let view = teams_view(&state);
    let team = &view.teams()[0];
    assert!(team.tasks.is_empty(), "the bot owns no card");
    assert_eq!(team.lead_name(), Some("Nora"));
    let bot = &team.bots[0];
    assert_eq!(
        (bot.role.as_str(), bot.agent),
        ("Reviews", Some(Agent::Claude))
    );
    stop_service(service).await;
}

#[tokio::test]
async fn a_bot_added_through_another_client_reaches_the_store_without_a_request() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let client = client_of(home.path()).await;
    let team = client
        .team_create(&TeamCreateParams {
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    let (_bridge, _handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first read", |state| {
        state.teams.is_some()
    })
    .await;

    client
        .team_bot_add(&nora(&team.id, false))
        .await
        .expect("the bot is added");
    until(&mut updates, &mut state, "the bot to be listed", |state| {
        teams_view(state)
            .teams()
            .iter()
            .any(|team| team.bots.iter().any(|bot| bot.name == "Nora"))
    })
    .await;
    stop_service(service).await;
}

#[tokio::test]
async fn a_team_project_is_listed_with_its_brief_and_its_own_board() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let client = client_of(home.path()).await;
    let team = client
        .team_create(&TeamCreateParams {
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    let project = client
        .team_project_add(&TeamProjectAddParams {
            team: team.id.clone(),
            goal: Some("Ship checkout".to_owned()),
            brief: Some("Checkout copy".to_owned()),
            ..base_team_project_add_params()
        })
        .await
        .expect("the project is added");
    client
        .task_create(&TaskCreateParams {
            team: team.id.clone(),
            title: "Draft the page".to_owned(),
            project: Some(project.id.clone()),
            ..base_task_create_params()
        })
        .await
        .expect("the task is created");

    let (_bridge, _handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(
        &mut updates,
        &mut state,
        "the project to be listed",
        |state| {
            teams_view(state).teams().iter().any(|team| {
                team.projects
                    .iter()
                    .any(|project| project.name == "Checkout")
            })
        },
    )
    .await;
    let view = teams_view(&state);
    let project = &view.teams()[0].projects[0];
    assert_eq!(project.brief, "Checkout copy");
    let titles: Vec<&str> = project
        .tasks
        .iter()
        .map(|task| task.title.as_str())
        .collect();
    assert_eq!(titles, ["Draft the page"]);
    stop_service(service).await;
}

#[tokio::test]
async fn a_team_project_added_through_another_client_reaches_the_store_without_a_request() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let client = client_of(home.path()).await;
    let team = client
        .team_create(&TeamCreateParams {
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    let (_bridge, _handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first read", |state| {
        state.teams.is_some()
    })
    .await;

    client
        .team_project_add(&TeamProjectAddParams {
            team: team.id.clone(),
            ..base_team_project_add_params()
        })
        .await
        .expect("the project is added");
    until(
        &mut updates,
        &mut state,
        "the project to be listed",
        |state| {
            teams_view(state).teams().iter().any(|team| {
                team.projects
                    .iter()
                    .any(|project| project.name == "Checkout")
            })
        },
    )
    .await;
    stop_service(service).await;
}

/// The service's ids of the team that lists the project `name`, and of that project, as the store numbered them.
fn keys_of(state: &StoreState, name: &str) -> (String, String) {
    let view = teams_view(state);
    let team = view
        .teams()
        .iter()
        .find(|team| team.projects.iter().any(|project| project.name == name))
        .expect("the project is listed");
    let project = team
        .projects
        .iter()
        .find(|project| project.name == name)
        .expect("the project is listed");
    let team_key = state
        .team_ids
        .key(team.id.0)
        .expect("the team is numbered")
        .to_owned();
    let project_key = state
        .team_ids
        .key(project.id.0)
        .expect("the project is numbered")
        .to_owned();
    (team_key, project_key)
}

/// A team named Ops with one project named Checkout, briefed "Old brief", and a bridge that has read it.
async fn team_with_checkout(
    home: &Path,
) -> (
    ServiceBridge,
    ServiceHandle,
    UnboundedReceiver<StoreUpdate>,
    StoreState,
) {
    let client = client_of(home).await;
    let team = client
        .team_create(&TeamCreateParams {
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    client
        .team_project_add(&TeamProjectAddParams {
            team: team.id.clone(),
            brief: Some("Old brief".to_owned()),
            ..base_team_project_add_params()
        })
        .await
        .expect("the project is added");
    let (bridge, handle, mut updates) = bridge_for(home);
    let mut state = StoreState::default();
    until(
        &mut updates,
        &mut state,
        "the project to be listed",
        |state| {
            teams_view(state).teams().iter().any(|team| {
                team.projects
                    .iter()
                    .any(|project| project.name == "Checkout")
            })
        },
    )
    .await;
    (bridge, handle, updates, state)
}

#[tokio::test]
async fn a_brief_saved_through_the_bridge_is_read_back() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates, mut state) = team_with_checkout(home.path()).await;
    let (team_key, project_key) = keys_of(&state, "Checkout");

    handle
        .save_brief(team_key, project_key, "New brief".to_owned())
        .await
        .expect("the brief is saved");
    until(
        &mut updates,
        &mut state,
        "the new brief to be read",
        |state| {
            teams_view(state).teams().iter().any(|team| {
                team.projects
                    .iter()
                    .any(|project| project.brief == "New brief")
            })
        },
    )
    .await;
    stop_service(service).await;
}

#[tokio::test]
async fn a_project_deleted_through_the_bridge_leaves_the_read_and_a_second_delete_is_refused() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates, mut state) = team_with_checkout(home.path()).await;
    let (team_key, project_key) = keys_of(&state, "Checkout");

    handle
        .delete_project(team_key.clone(), project_key.clone())
        .await
        .expect("the project is deleted");
    until(
        &mut updates,
        &mut state,
        "the project to leave the read",
        |state| {
            teams_view(state)
                .teams()
                .iter()
                .all(|team| team.projects.is_empty())
        },
    )
    .await;
    let refusal = handle
        .delete_project(team_key, project_key)
        .await
        .expect_err("a project that is gone is refused");
    assert!(
        matches!(&refusal, Refusal::Failed { message, .. } if message.contains("gone")),
        "{refusal:?}"
    );
    stop_service(service).await;
}

#[tokio::test]
async fn a_team_access_change_reaches_the_service_and_the_read_shows_it() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let client = client_of(home.path()).await;
    let team = client
        .team_create(&TeamCreateParams {
            name: "Desarrollo".to_owned(),
            access: Some(TeamAccess::Full),
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");

    let (_bridge, handle, mut updates) = bridge_for(home.path());
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

    let changed = handle
        .set_team_access(team.id.clone(), TeamAccess::Default)
        .await
        .expect("the owner may change the access");
    assert_eq!(changed.access, TeamAccess::Default);
    until(
        &mut updates,
        &mut state,
        "the new access in the read",
        |state| {
            state.teams.as_deref().is_some_and(|records| {
                records
                    .iter()
                    .any(|record| record.team.access == TeamAccess::Default)
            })
        },
    )
    .await;
    let stored = client
        .team_show(&TeamRef {
            team: team.id.clone(),
        })
        .await
        .expect("the team is read");
    assert_eq!(stored.access, TeamAccess::Default);
    stop_service(service).await;
}

/// The service's id of the team named `name`, as the store numbered it.
fn team_key_named(state: &StoreState, name: &str) -> String {
    let view = teams_view(state);
    let team = view
        .teams()
        .iter()
        .find(|team| team.name == name)
        .expect("the team is listed");
    state
        .team_ids
        .key(team.id.0)
        .expect("the team is numbered")
        .to_owned()
}

#[tokio::test]
async fn a_team_update_through_the_bridge_changes_only_the_fields_given() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first read", |state| {
        state.teams.is_some()
    })
    .await;
    handle
        .create_team(TeamCreateParams {
            purpose: Some("Run it".to_owned()),
            workspaces: Some(false),
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    until(&mut updates, &mut state, "the team to be read", |state| {
        teams_view(state)
            .teams()
            .iter()
            .any(|team| team.name == "Ops")
    })
    .await;

    let renamed = handle
        .update_team(TeamUpdateParams {
            team: team_key_named(&state, "Ops"),
            name: Some("Platform".to_owned()),
            ..base_team_update_params()
        })
        .await
        .expect("the team is renamed");
    assert_eq!(
        (
            renamed.name.as_str(),
            renamed.purpose.as_str(),
            renamed.workspaces
        ),
        ("Platform", "Run it", false)
    );
    until(
        &mut updates,
        &mut state,
        "the new name to be read",
        |state| {
            teams_view(state)
                .teams()
                .iter()
                .any(|team| team.name == "Platform")
        },
    )
    .await;
    stop_service(service).await;
}

#[tokio::test]
async fn a_team_renamed_to_a_name_another_team_has_is_refused_as_a_name_problem() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first read", |state| {
        state.teams.is_some()
    })
    .await;
    for name in ["Ops", "Dev"] {
        handle
            .create_team(TeamCreateParams {
                name: name.to_owned(),
                ..base_team_create_params()
            })
            .await
            .expect("the team is created");
    }
    until(&mut updates, &mut state, "both teams to be read", |state| {
        teams_view(state).teams().len() == 2
    })
    .await;

    let refusal = handle
        .update_team(TeamUpdateParams {
            team: team_key_named(&state, "Dev"),
            name: Some("Ops".to_owned()),
            ..base_team_update_params()
        })
        .await
        .expect_err("the name is taken");
    assert!(
        matches!(&refusal, Refusal::NameProblem { message } if message.contains("another team")),
        "{refusal:?}"
    );
    stop_service(service).await;
}

#[tokio::test]
async fn a_team_removed_through_the_bridge_leaves_the_read_and_a_second_removal_is_refused() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first read", |state| {
        state.teams.is_some()
    })
    .await;
    handle
        .create_team(TeamCreateParams {
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    until(&mut updates, &mut state, "the team to be read", |state| {
        teams_view(state)
            .teams()
            .iter()
            .any(|team| team.name == "Ops")
    })
    .await;
    let key = team_key_named(&state, "Ops");

    let removed = handle
        .remove_team(key.clone())
        .await
        .expect("the team is removed");
    assert_eq!(removed.name, "Ops");
    until(
        &mut updates,
        &mut state,
        "the team to leave the read",
        |state| teams_view(state).teams().is_empty(),
    )
    .await;
    let refusal = handle
        .remove_team(key)
        .await
        .expect_err("a team that is gone is refused");
    assert!(
        matches!(&refusal, Refusal::Failed { message, .. } if message.contains("gone")),
        "{refusal:?}"
    );
    stop_service(service).await;
}

#[tokio::test]
async fn a_team_project_added_through_the_bridge_is_stored_and_a_refused_one_is_not() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first read", |state| {
        state.teams.is_some()
    })
    .await;
    handle
        .create_team(TeamCreateParams {
            workspaces: Some(false),
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    until(&mut updates, &mut state, "the team to be read", |state| {
        teams_view(state)
            .teams()
            .iter()
            .any(|team| team.name == "Ops")
    })
    .await;
    let team = team_key_named(&state, "Ops");

    // A code source needs Workspaces in the team, so the service refuses this project and stores nothing.
    let refusal = handle
        .add_team_project(TeamProjectAddParams {
            team: team.clone(),
            code_source: Some(CodeSource::ExistingRepository),
            ..base_team_project_add_params()
        })
        .await
        .expect_err("a code source needs Workspaces");
    assert!(
        matches!(&refusal, Refusal::Failed { message, .. } if message.contains("Workspaces")),
        "{refusal:?}"
    );

    let project = handle
        .add_team_project(TeamProjectAddParams {
            team: team.clone(),
            goal: Some("Ship the cart".to_owned()),
            ..base_team_project_add_params()
        })
        .await
        .expect("the project is added");
    assert_eq!(project.name, "Checkout");
    until(
        &mut updates,
        &mut state,
        "the project to be read",
        |state| {
            teams_view(state).teams().iter().any(|team| {
                team.projects
                    .iter()
                    .any(|project| project.name == "Checkout")
            })
        },
    )
    .await;
    let listed = client_of(home.path())
        .await
        .team_project_list(&TeamRef { team })
        .await
        .expect("the projects are listed");
    assert_eq!(
        listed.projects.len(),
        1,
        "the refused project is not stored"
    );
    assert_eq!(listed.projects[0].goal, "Ship the cart");
    stop_service(service).await;
}

/// The team and bot keys of the bot named `name`, as the store numbered them.
fn bot_keys(state: &StoreState, name: &str) -> (String, String) {
    let view = teams_view(state);
    let team = view
        .teams()
        .iter()
        .find(|team| team.bots.iter().any(|bot| bot.name == name))
        .expect("the bot is listed");
    let bot = team
        .bots
        .iter()
        .find(|bot| bot.name == name)
        .expect("the bot is listed");
    let team_key = state
        .team_ids
        .key(team.id.0)
        .expect("the team is numbered")
        .to_owned();
    let bot_key = state
        .team_ids
        .key(bot.id.0)
        .expect("the bot is numbered")
        .to_owned();
    (team_key, bot_key)
}

#[tokio::test]
async fn a_bot_renamed_through_the_bridge_keeps_its_job_and_one_with_open_work_is_refused() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let client = client_of(home.path()).await;
    let team = client
        .team_create(&TeamCreateParams {
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    let bot = client
        .team_bot_add(&nora(&team.id, false))
        .await
        .expect("the bot is added");
    client
        .task_create(&TaskCreateParams {
            team: team.id.clone(),
            title: "Review the diff".to_owned(),
            bot: Some(bot.id.clone()),
            ..base_task_create_params()
        })
        .await
        .expect("the task is created");

    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the bot to be listed", |state| {
        teams_view(state)
            .teams()
            .iter()
            .any(|team| team.bots.iter().any(|bot| bot.name == "Nora"))
    })
    .await;
    let (team_key, bot_key) = bot_keys(&state, "Nora");

    let refusal = handle
        .remove_bot(BotRemoveParams {
            team: team_key.clone(),
            bot: bot_key.clone(),
            ..base_bot_remove_params()
        })
        .await
        .expect_err("a bot that owns an open task is refused");
    assert!(
        matches!(&refusal, Refusal::Failed { message, .. } if message.contains("open task")),
        "{refusal:?}"
    );

    let renamed = handle
        .update_bot(BotUpdateParams {
            team: team_key,
            bot: bot_key,
            name: Some("Lucia".to_owned()),
            ..base_bot_update_params()
        })
        .await
        .expect("the bot is renamed");
    assert_eq!(
        (renamed.name.as_str(), renamed.job.as_str()),
        ("Lucia", "Reviews")
    );
    until(
        &mut updates,
        &mut state,
        "the new name to be read",
        |state| {
            teams_view(state).teams().iter().any(|team| {
                team.bots
                    .iter()
                    .any(|bot| bot.name == "Lucia" && bot.role == "Reviews")
            })
        },
    )
    .await;
    stop_service(service).await;
}

#[tokio::test]
async fn a_bot_without_open_work_is_removed_through_the_bridge_and_a_second_removal_is_refused() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let client = client_of(home.path()).await;
    let team = client
        .team_create(&TeamCreateParams {
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    client
        .team_bot_add(&nora(&team.id, false))
        .await
        .expect("the bot is added");

    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the bot to be listed", |state| {
        teams_view(state)
            .teams()
            .iter()
            .any(|team| team.bots.iter().any(|bot| bot.name == "Nora"))
    })
    .await;
    let (team_key, bot_key) = bot_keys(&state, "Nora");

    let removed = handle
        .remove_bot(BotRemoveParams {
            team: team_key.clone(),
            bot: bot_key.clone(),
            ..base_bot_remove_params()
        })
        .await
        .expect("the bot is removed");
    assert_eq!(removed.name, "Nora");
    until(
        &mut updates,
        &mut state,
        "the bot to leave the read",
        |state| {
            teams_view(state)
                .teams()
                .iter()
                .all(|team| team.bots.is_empty())
        },
    )
    .await;
    let refusal = handle
        .remove_bot(BotRemoveParams {
            team: team_key,
            bot: bot_key,
            ..base_bot_remove_params()
        })
        .await
        .expect_err("a bot that is gone is refused");
    assert!(
        matches!(&refusal, Refusal::Failed { message, .. } if message.contains("gone")),
        "{refusal:?}"
    );
    stop_service(service).await;
}

/// The protocol id of the first card the store shows, as the bridge numbers it.
fn first_card_key(state: &StoreState) -> String {
    let view = teams_view(state);
    let task = view.teams()[0].tasks[0].id;
    state
        .team_ids
        .key(task.0)
        .expect("the card is numbered")
        .to_owned()
}

/// A team named Ops with one card that has no bot. Returns a client of the service and the team's id.
async fn team_with_an_unassigned_card(home: &Path) -> (Client, String) {
    let client = client_of(home).await;
    let team = client
        .team_create(&TeamCreateParams {
            ..base_team_create_params()
        })
        .await
        .expect("the team is created");
    client
        .task_create(&TaskCreateParams {
            team: team.id.clone(),
            ..base_task_create_params()
        })
        .await
        .expect("the task is created");
    (client, team.id)
}

/// How many cards the service's Board holds in Open.
async fn open_cards(client: &Client, team: &str) -> usize {
    let board = client
        .team_board(&BoardParams {
            team: team.to_owned(),
            ..base_board_params()
        })
        .await
        .expect("the Board is read");
    assert_eq!(board.columns[0].column, Column::Open);
    board.columns[0].cards.len()
}

#[tokio::test]
async fn a_card_without_a_bot_is_refused_its_start_in_the_services_words() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (client, team) = team_with_an_unassigned_card(home.path()).await;

    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the card", |state| {
        teams_view(state)
            .teams()
            .iter()
            .any(|listed| !listed.tasks.is_empty())
    })
    .await;
    let refusal = handle
        .start_task(first_card_key(&state))
        .await
        .expect_err("a card without a bot does not start");
    assert!(
        refusal.message().contains("has no bot"),
        "{}",
        refusal.message()
    );
    assert_eq!(
        open_cards(&client, &team).await,
        1,
        "the card stays in Open"
    );
    stop_service(service).await;
}

#[tokio::test]
async fn a_card_with_no_run_is_not_stopped_and_the_service_says_so() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (client, team) = team_with_an_unassigned_card(home.path()).await;

    let (_bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the card", |state| {
        teams_view(state)
            .teams()
            .iter()
            .any(|listed| !listed.tasks.is_empty())
    })
    .await;
    let refusal = handle
        .stop_task(first_card_key(&state))
        .await
        .expect_err("a card with no run has nothing to stop");
    assert!(
        refusal.message().contains("is not running"),
        "{}",
        refusal.message()
    );
    assert_eq!(
        open_cards(&client, &team).await,
        1,
        "the card stays in Open"
    );
    stop_service(service).await;
}

/// Every `PeerInfo` field at its usual value; tests override the fields they vary.
fn base_peer_info() -> massorca_protocol::PeerInfo {
    massorca_protocol::PeerInfo {
        name: "massorca-test-client".to_owned(),
        version: "0.0.0-test".to_owned(),
    }
}

/// Every `BoardParams` field at its usual value; tests override the fields they vary.
fn base_board_params() -> massorca_protocol::methods::teams::BoardParams {
    massorca_protocol::methods::teams::BoardParams {
        team: String::new(),
        project: None,
    }
}

/// Every `TaskCreateParams` field at its usual value; tests override the fields they vary.
fn base_task_create_params() -> massorca_protocol::methods::tasks::TaskCreateParams {
    massorca_protocol::methods::tasks::TaskCreateParams {
        team: String::new(),
        title: "Rotate the keys".to_owned(),
        body: None,
        bot: None,
        project: None,
        origin: None,
    }
}

/// Every `TeamCreateParams` field at its usual value; tests override the fields they vary.
fn base_team_create_params() -> massorca_protocol::methods::teams::TeamCreateParams {
    massorca_protocol::methods::teams::TeamCreateParams {
        name: "Ops".to_owned(),
        purpose: None,
        character: None,
        access: None,
        workspaces: None,
    }
}

/// Every `TeamProjectAddParams` field at its usual value; tests override the fields they vary.
fn base_team_project_add_params() -> massorca_protocol::methods::teams::TeamProjectAddParams {
    massorca_protocol::methods::teams::TeamProjectAddParams {
        team: String::new(),
        name: "Checkout".to_owned(),
        goal: None,
        brief: None,
        source_project: None,
        code_source: None,
    }
}

/// Every `TeamUpdateParams` field at its usual value; tests override the fields they vary.
fn base_team_update_params() -> massorca_protocol::methods::teams::TeamUpdateParams {
    massorca_protocol::methods::teams::TeamUpdateParams {
        team: String::new(),
        name: None,
        purpose: None,
        character: None,
        access: None,
        workspaces: None,
    }
}

/// Every `BotAddParams` field at its usual value; tests override the fields they vary.
fn base_bot_add_params() -> massorca_protocol::methods::teams::BotAddParams {
    massorca_protocol::methods::teams::BotAddParams {
        team: String::new(),
        name: "Nora".to_owned(),
        job: "Reviews".to_owned(),
        kind: "claude".to_owned(),
        model: None,
        effort: None,
        lead: false,
    }
}

/// Every `BotRemoveParams` field at its usual value; tests override the fields they vary.
fn base_bot_remove_params() -> massorca_protocol::methods::teams::BotRemoveParams {
    massorca_protocol::methods::teams::BotRemoveParams {
        team: String::new(),
        bot: String::new(),
        unassign: false,
    }
}

/// Every `BotUpdateParams` field at its usual value; tests override the fields they vary.
fn base_bot_update_params() -> massorca_protocol::methods::teams::BotUpdateParams {
    massorca_protocol::methods::teams::BotUpdateParams {
        team: String::new(),
        bot: String::new(),
        name: None,
        job: None,
        kind: None,
        model: None,
        effort: None,
        lead: None,
    }
}
