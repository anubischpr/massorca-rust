//! Teams, bots and team projects over the wire: their CRUD, name and lead rules, refusals and events.

use std::time::Duration;

use massorca_client::Subscription;
use massorca_protocol::codes;
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_TEAM_CHANGED};
use massorca_protocol::methods::teams::{
    self, Bot, BotAddParams, BotRemoveParams, BotUpdateParams, Team, TeamAccess, TeamCharacter,
    TeamCreateParams, TeamList, TeamProject, TeamProjectAddParams, TeamProjectRemoveParams,
    TeamProjectUpdateParams, TeamRef, TeamRemoveParams, TeamUpdateParams,
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

fn bot_params(team: &str, name: &str, lead: bool) -> BotAddParams {
    BotAddParams {
        team: team.to_owned(),
        name: name.to_owned(),
        job: "Writes the parser".to_owned(),
        kind: "claude".to_owned(),
        model: None,
        effort: None,
        lead,
    }
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
async fn bots_are_added_updated_and_removed_and_their_names_are_unique_within_a_team() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let alpha: Team = client
        .call(teams::CREATE, &create_params("alpha"))
        .await
        .expect("alpha");
    let beta: Team = client
        .call(teams::CREATE, &create_params("beta"))
        .await
        .expect("beta");

    let mara: Bot = client
        .call(teams::BOT_ADD, &bot_params("alpha", "mara", false))
        .await
        .expect("team.bot_add");
    assert_eq!(mara.team_id, alpha.id);
    assert_eq!(mara.kind, "claude");
    assert!(!mara.lead);
    client
        .call::<_, Bot>(teams::BOT_ADD, &bot_params("beta", "mara", false))
        .await
        .expect("another team may use the name");
    let taken = client
        .call::<_, Bot>(teams::BOT_ADD, &bot_params("alpha", "mara", false))
        .await
        .expect_err("a name taken in the team");
    assert_eq!(remote_code(&taken), codes::BOT_NAME_IN_USE);

    let updated: Bot = client
        .call(
            teams::BOT_UPDATE,
            &BotUpdateParams {
                team: "alpha".to_owned(),
                bot: mara.id.clone(),
                name: None,
                job: Some("Reviews the parser".to_owned()),
                kind: None,
                model: Some("opus".to_owned()),
                effort: Some("high".to_owned()),
                lead: None,
            },
        )
        .await
        .expect("team.bot_update");
    assert_eq!(updated.job, "Reviews the parser");
    assert_eq!(updated.model.as_deref(), Some("opus"));
    assert_eq!(updated.effort.as_deref(), Some("high"));

    let cleared: Bot = client
        .call(
            teams::BOT_UPDATE,
            &BotUpdateParams {
                team: "alpha".to_owned(),
                bot: "mara".to_owned(),
                name: None,
                job: None,
                kind: None,
                model: Some(String::new()),
                effort: None,
                lead: None,
            },
        )
        .await
        .expect("an empty model clears it");
    assert_eq!(cleared.model, None);
    assert_eq!(cleared.effort.as_deref(), Some("high"));

    client
        .call::<_, Bot>(teams::BOT_ADD, &bot_params("alpha", "bruno", false))
        .await
        .expect("bruno");
    let renamed_taken = client
        .call::<_, Bot>(
            teams::BOT_UPDATE,
            &BotUpdateParams {
                team: "alpha".to_owned(),
                bot: mara.id.clone(),
                name: Some("bruno".to_owned()),
                job: None,
                kind: None,
                model: None,
                effort: None,
                lead: None,
            },
        )
        .await
        .expect_err("a rename onto a taken name");
    assert_eq!(remote_code(&renamed_taken), codes::BOT_NAME_IN_USE);

    let removed: Bot = client
        .call(
            teams::BOT_REMOVE,
            &BotRemoveParams {
                team: "alpha".to_owned(),
                bot: "mara".to_owned(),
                unassign: false,
            },
        )
        .await
        .expect("team.bot_rm");
    assert_eq!(removed.id, mara.id);
    let gone = client
        .call::<_, Bot>(
            teams::BOT_REMOVE,
            &BotRemoveParams {
                team: "alpha".to_owned(),
                bot: "mara".to_owned(),
                unassign: false,
            },
        )
        .await
        .expect_err("a removed bot is gone");
    assert_eq!(remote_code(&gone), codes::BOT_NOT_FOUND);
    assert_eq!(beta.name, "beta");
    stop(running).await;
}

#[tokio::test]
async fn a_team_has_one_lead_and_the_lead_moves_only_by_clearing_it_first() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &create_params("alpha"))
        .await
        .expect("alpha");
    let mara: Bot = client
        .call(teams::BOT_ADD, &bot_params("alpha", "mara", true))
        .await
        .expect("the first lead");
    assert!(mara.lead);

    let second_lead = client
        .call::<_, Bot>(teams::BOT_ADD, &bot_params("alpha", "bruno", true))
        .await
        .expect_err("a second lead is refused");
    assert_eq!(remote_code(&second_lead), codes::BOT_LEAD_TAKEN);
    client
        .call::<_, Bot>(teams::BOT_ADD, &bot_params("alpha", "bruno", false))
        .await
        .expect("bruno joins as a bot");

    let promote = client
        .call::<_, Bot>(
            teams::BOT_UPDATE,
            &BotUpdateParams {
                team: "alpha".to_owned(),
                bot: "bruno".to_owned(),
                name: None,
                job: None,
                kind: None,
                model: None,
                effort: None,
                lead: Some(true),
            },
        )
        .await
        .expect_err("promotion while mara leads is refused");
    assert_eq!(remote_code(&promote), codes::BOT_LEAD_TAKEN);

    client
        .call::<_, Bot>(
            teams::BOT_UPDATE,
            &BotUpdateParams {
                team: "alpha".to_owned(),
                bot: "mara".to_owned(),
                name: None,
                job: None,
                kind: None,
                model: None,
                effort: None,
                lead: Some(false),
            },
        )
        .await
        .expect("clearing the lead");
    let moved: Bot = client
        .call(
            teams::BOT_UPDATE,
            &BotUpdateParams {
                team: "alpha".to_owned(),
                bot: "bruno".to_owned(),
                name: None,
                job: None,
                kind: None,
                model: None,
                effort: None,
                lead: Some(true),
            },
        )
        .await
        .expect("bruno leads once mara has stepped down");
    assert!(moved.lead);
    stop(running).await;
}

#[tokio::test]
async fn team_projects_are_added_updated_and_removed_within_their_team() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &create_params("alpha"))
        .await
        .expect("alpha");

    let parser: TeamProject = client
        .call(
            teams::PROJECT_ADD,
            &TeamProjectAddParams {
                team: "alpha".to_owned(),
                name: "parser".to_owned(),
                goal: Some("Parse the config".to_owned()),
                brief: None,
            },
        )
        .await
        .expect("team.project_add");
    assert_eq!(parser.goal, "Parse the config");
    assert_eq!(parser.brief, "");

    let taken = client
        .call::<_, TeamProject>(
            teams::PROJECT_ADD,
            &TeamProjectAddParams {
                team: "alpha".to_owned(),
                name: "parser".to_owned(),
                goal: None,
                brief: None,
            },
        )
        .await
        .expect_err("a name taken in the team");
    assert_eq!(remote_code(&taken), codes::TEAM_PROJECT_NAME_IN_USE);

    let updated: TeamProject = client
        .call(
            teams::PROJECT_UPDATE,
            &TeamProjectUpdateParams {
                team: "alpha".to_owned(),
                project: parser.id.clone(),
                name: None,
                goal: None,
                brief: Some("Keep it small.".to_owned()),
            },
        )
        .await
        .expect("team.project_update");
    assert_eq!(updated.brief, "Keep it small.");
    assert_eq!(updated.name, "parser");

    let removed: TeamProject = client
        .call(
            teams::PROJECT_REMOVE,
            &TeamProjectRemoveParams {
                team: "alpha".to_owned(),
                project: "parser".to_owned(),
            },
        )
        .await
        .expect("team.project_rm");
    assert_eq!(removed.id, parser.id);
    let gone = client
        .call::<_, TeamProject>(
            teams::PROJECT_REMOVE,
            &TeamProjectRemoveParams {
                team: "alpha".to_owned(),
                project: parser.id.clone(),
            },
        )
        .await
        .expect_err("a removed project is gone");
    assert_eq!(remote_code(&gone), codes::TEAM_PROJECT_NOT_FOUND);
    stop(running).await;
}

#[tokio::test]
async fn unknown_teams_bots_and_projects_are_refused_with_typed_codes() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &create_params("alpha"))
        .await
        .expect("alpha");
    client
        .call::<_, Team>(teams::CREATE, &create_params("beta"))
        .await
        .expect("beta");
    let beta_bot: Bot = client
        .call(teams::BOT_ADD, &bot_params("beta", "bruno", false))
        .await
        .expect("bruno in beta");

    let no_team = client
        .call::<_, Bot>(teams::BOT_ADD, &bot_params("nothing", "mara", false))
        .await
        .expect_err("an unknown team");
    assert_eq!(remote_code(&no_team), codes::TEAM_NOT_FOUND);

    let other_teams_bot = client
        .call::<_, Bot>(
            teams::BOT_UPDATE,
            &BotUpdateParams {
                team: "alpha".to_owned(),
                bot: beta_bot.id.clone(),
                name: None,
                job: Some("Edits".to_owned()),
                kind: None,
                model: None,
                effort: None,
                lead: None,
            },
        )
        .await
        .expect_err("a bot of another team is not in this team");
    assert_eq!(remote_code(&other_teams_bot), codes::BOT_NOT_FOUND);

    let no_project = client
        .call::<_, TeamProject>(
            teams::PROJECT_UPDATE,
            &TeamProjectUpdateParams {
                team: "alpha".to_owned(),
                project: "missing".to_owned(),
                name: Some("renamed".to_owned()),
                goal: None,
                brief: None,
            },
        )
        .await
        .expect_err("an unknown project");
    assert_eq!(remote_code(&no_project), codes::TEAM_PROJECT_NOT_FOUND);
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
        .call::<_, Bot>(teams::BOT_ADD, &bot_params("alpha", "mara", false))
        .await
        .expect("team.bot_add");
    assert_eq!(next_event(&mut events).await.id, alpha.id);

    client
        .call::<_, TeamProject>(
            teams::PROJECT_ADD,
            &TeamProjectAddParams {
                team: "alpha".to_owned(),
                name: "parser".to_owned(),
                goal: None,
                brief: None,
            },
        )
        .await
        .expect("team.project_add");
    assert_eq!(next_event(&mut events).await.id, alpha.id);

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
