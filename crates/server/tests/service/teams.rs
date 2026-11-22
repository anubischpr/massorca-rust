//! Teams over the wire: their CRUD, their name rule, their refusals and their events.

use std::time::Duration;

use massorca_client::Subscription;
use massorca_protocol::codes;
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_TEAM_CHANGED};
use massorca_protocol::methods::teams::{
    self, Team, TeamAccess, TeamCharacter, TeamCreateParams, TeamList, TeamRef, TeamRemoveParams,
    TeamUpdateParams,
};
use serde_json::Value;
use tokio::time::timeout;

use super::common::{connect, remote_code, start, stop, temp_home};

fn create_params(name: &str) -> TeamCreateParams {
    TeamCreateParams {
        name: name.to_owned(),
        purpose: None,
        character: None,
        access: None,
        workspaces: None,
    }
}

/// The next change event on `events`, within five seconds.
async fn next_event(events: &mut Subscription) -> ChangeEvent {
    timeout(Duration::from_secs(5), events.next_as::<ChangeEvent>())
        .await
        .expect("the event arrives")
        .expect("stream open")
        .expect("event decodes")
}

#[tokio::test]
async fn a_team_is_created_with_its_defaults_and_listed_shown_updated_and_removed() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let created: Team = client
        .call(teams::CREATE, &create_params("alpha"))
        .await
        .expect("team.create");
    assert_eq!(created.name, "alpha");
    assert_eq!(created.purpose, "");
    assert_eq!(created.character, TeamCharacter::Triangle);
    assert_eq!(created.access, TeamAccess::Full);
    assert!(!created.workspaces);

    let list: TeamList = client
        .call(teams::LIST, &Value::Null)
        .await
        .expect("team.list");
    assert_eq!(list.teams, vec![created.clone()]);

    let by_name: Team = client
        .call(
            teams::SHOW,
            &TeamRef {
                team: "alpha".to_owned(),
            },
        )
        .await
        .expect("team.show by name");
    assert_eq!(by_name, created);

    let updated: Team = client
        .call(
            teams::UPDATE,
            &TeamUpdateParams {
                team: created.id.clone(),
                name: Some("beta".to_owned()),
                purpose: Some("Owns the parser".to_owned()),
                character: Some(TeamCharacter::Square),
                access: None,
                workspaces: Some(true),
            },
        )
        .await
        .expect("team.update by id");
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.name, "beta");
    assert_eq!(updated.purpose, "Owns the parser");
    assert_eq!(updated.character, TeamCharacter::Square);
    assert!(updated.workspaces);
    assert_eq!(updated.created_at_ms, created.created_at_ms);

    let removed: Team = client
        .call(
            teams::REMOVE,
            &TeamRemoveParams {
                team: "beta".to_owned(),
                force: false,
            },
        )
        .await
        .expect("team.rm");
    assert_eq!(removed.id, created.id);
    let missing = client
        .call::<_, Team>(
            teams::SHOW,
            &TeamRef {
                team: created.id.clone(),
            },
        )
        .await
        .expect_err("a removed team is gone");
    assert_eq!(remote_code(&missing), codes::TEAM_NOT_FOUND);
    stop(running).await;
}

#[tokio::test]
async fn team_names_are_unique_and_a_team_may_keep_its_own_name() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &create_params("alpha"))
        .await
        .expect("first team");
    let beta: Team = client
        .call(teams::CREATE, &create_params("beta"))
        .await
        .expect("second team");

    let taken = client
        .call::<_, Team>(teams::CREATE, &create_params("alpha"))
        .await
        .expect_err("a name in use");
    assert_eq!(remote_code(&taken), codes::TEAM_NAME_IN_USE);

    let renamed_to_taken = client
        .call::<_, Team>(
            teams::UPDATE,
            &TeamUpdateParams {
                team: beta.id.clone(),
                name: Some("alpha".to_owned()),
                purpose: None,
                character: None,
                access: None,
                workspaces: None,
            },
        )
        .await
        .expect_err("a rename onto a taken name");
    assert_eq!(remote_code(&renamed_to_taken), codes::TEAM_NAME_IN_USE);

    let same_name: Team = client
        .call(
            teams::UPDATE,
            &TeamUpdateParams {
                team: beta.id.clone(),
                name: Some("beta".to_owned()),
                purpose: None,
                character: None,
                access: None,
                workspaces: None,
            },
        )
        .await
        .expect("keeping its own name is not a conflict");
    assert_eq!(same_name.name, "beta");
    stop(running).await;
}

#[tokio::test]
async fn a_blank_team_name_is_refused_as_invalid_params() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let blank = client
        .call::<_, Team>(teams::CREATE, &create_params("   "))
        .await
        .expect_err("a blank name");
    assert_eq!(remote_code(&blank), codes::REQUEST_INVALID_PARAMS);
    stop(running).await;
}

#[tokio::test]
async fn team_changes_publish_team_changed_with_the_team_id() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let mut events = client.subscribe_events().await.expect("subscribe");

    let alpha: Team = client
        .call(teams::CREATE, &create_params("alpha"))
        .await
        .expect("team.create");
    assert_eq!(
        next_event(&mut events).await,
        ChangeEvent {
            topic: TOPIC_TEAM_CHANGED.to_owned(),
            id: alpha.id.clone()
        }
    );

    client
        .call::<_, Team>(
            teams::REMOVE,
            &TeamRemoveParams {
                team: alpha.id.clone(),
                force: false,
            },
        )
        .await
        .expect("team.rm");
    assert_eq!(next_event(&mut events).await.topic, TOPIC_TEAM_CHANGED);
    stop(running).await;
}
