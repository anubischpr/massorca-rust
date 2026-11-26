//! Teams, bots and team projects over the wire: their CRUD, name and lead rules, refusals and events.

use std::path::Path;
use std::time::Duration;

use massorca_client::{Client, Subscription};
use massorca_protocol::codes;
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_TEAM_CHANGED};
use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectKind, ProjectRef,
};
use massorca_protocol::methods::tasks::{self, Task, TaskCreateParams};
use massorca_protocol::methods::teams::{
    self, Bot, BotAddParams, BotList, BotRemoveParams, BotUpdateParams, CodeSource, MemorySource,
    Team, TeamAccess, TeamCharacter, TeamCreateParams, TeamList, TeamMemoryAddParams,
    TeamMemoryLine, TeamMemoryList, TeamMemoryListParams, TeamMemoryRemoveParams, TeamProject,
    TeamProjectAddParams, TeamProjectList, TeamProjectRemoveParams, TeamProjectUpdateParams,
    TeamRef, TeamRemoveParams, TeamUpdateParams,
};
use serde_json::Value;
use tokio::time::timeout;

use super::common::{connect, remote_code, start, stop, temp_home};
use super::git_fixture::git;

/// A team with Workspaces on, which may connect its projects to code.
fn workspaces_team(name: &str) -> TeamCreateParams {
    TeamCreateParams {
        workspaces: Some(true),
        ..create_params(name)
    }
}

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
    timeout(super::common::REPLY_WAIT, events.next_as::<ChangeEvent>())
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
        skills: Vec::new(),
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
                skills: None,
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
                skills: None,
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
                skills: None,
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
                skills: None,
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
                skills: None,
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
                skills: None,
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
                source_project: None,
                code_source: None,
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
                source_project: None,
                code_source: None,
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
                source_project: None,
                code_source: None,
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
                skills: None,
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
                source_project: None,
                code_source: None,
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
                source_project: None,
                code_source: None,
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

fn team_ref(team: &str) -> TeamRef {
    TeamRef {
        team: team.to_owned(),
    }
}

#[tokio::test]
async fn a_teams_bots_and_projects_are_listed_in_creation_order_and_only_its_own() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let alpha: Team = client
        .call(teams::CREATE, &create_params("alpha"))
        .await
        .expect("alpha");
    client
        .call::<_, Team>(teams::CREATE, &create_params("beta"))
        .await
        .expect("beta");

    let empty: BotList = client
        .call(teams::BOT_LIST, &team_ref("alpha"))
        .await
        .expect("team.bot_list");
    assert!(empty.bots.is_empty());
    let no_projects: TeamProjectList = client
        .call(teams::PROJECT_LIST, &team_ref("alpha"))
        .await
        .expect("team.project_list");
    assert!(no_projects.projects.is_empty());

    for (team, name, lead) in [
        ("alpha", "zed", true),
        ("beta", "other", false),
        ("alpha", "ada", false),
    ] {
        client
            .call::<_, Bot>(teams::BOT_ADD, &bot_params(team, name, lead))
            .await
            .expect("team.bot_add");
    }
    for (team, name) in [
        ("alpha", "parser"),
        ("beta", "elsewhere"),
        ("alpha", "docs"),
    ] {
        client
            .call::<_, TeamProject>(
                teams::PROJECT_ADD,
                &TeamProjectAddParams {
                    team: team.to_owned(),
                    name: name.to_owned(),
                    goal: Some(format!("Ships the {name}")),
                    brief: None,
                    source_project: None,
                    code_source: None,
                },
            )
            .await
            .expect("team.project_add");
    }

    let bots: BotList = client
        .call(teams::BOT_LIST, &team_ref(&alpha.id))
        .await
        .expect("team.bot_list by id");
    let names: Vec<&str> = bots.bots.iter().map(|bot| bot.name.as_str()).collect();
    assert_eq!(names, ["zed", "ada"]);
    assert!(bots.bots[0].lead && !bots.bots[1].lead);
    assert!(bots.bots.iter().all(|bot| bot.team_id == alpha.id));

    let projects: TeamProjectList = client
        .call(teams::PROJECT_LIST, &team_ref("alpha"))
        .await
        .expect("team.project_list by name");
    let names: Vec<&str> = projects
        .projects
        .iter()
        .map(|project| project.name.as_str())
        .collect();
    assert_eq!(names, ["parser", "docs"]);
    assert_eq!(projects.projects[0].goal, "Ships the parser");

    let missing = client
        .call::<_, BotList>(teams::BOT_LIST, &team_ref("gamma"))
        .await
        .expect_err("an unknown team");
    assert_eq!(remote_code(&missing), codes::TEAM_NOT_FOUND);
    let missing = client
        .call::<_, TeamProjectList>(teams::PROJECT_LIST, &team_ref("gamma"))
        .await
        .expect_err("an unknown team");
    assert_eq!(remote_code(&missing), codes::TEAM_NOT_FOUND);
    stop(running).await;
}

fn memory_add(team: &str, text: &str) -> TeamMemoryAddParams {
    TeamMemoryAddParams {
        team: team.to_owned(),
        bot: None,
        text: text.to_owned(),
        task: None,
    }
}

fn memory_ids(lines: &[TeamMemoryLine]) -> Vec<String> {
    lines.iter().map(|line| line.id.clone()).collect()
}

#[tokio::test]
async fn team_memory_lines_are_added_listed_newest_first_per_scope_and_removed() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let alpha: Team = client
        .call(teams::CREATE, &create_params("alpha"))
        .await
        .expect("alpha");
    let mara: Bot = client
        .call(teams::BOT_ADD, &bot_params("alpha", "mara", false))
        .await
        .expect("team.bot_add");
    let task: Task = client
        .call(
            tasks::CREATE,
            &TaskCreateParams {
                team: "alpha".to_owned(),
                title: "Parse the config".to_owned(),
                body: None,
                bot: Some("mara".to_owned()),
                project: None,
                origin: None,
            },
        )
        .await
        .expect("task.create");

    let rule: TeamMemoryLine = client
        .call(
            teams::MEMORY_ADD,
            &memory_add("alpha", "  Keep the parser small.  "),
        )
        .await
        .expect("team.memory_add");
    assert_eq!(rule.team_id, alpha.id);
    assert_eq!(rule.bot_id, None);
    assert_eq!(rule.text, "Keep the parser small.");
    assert_eq!(rule.source, MemorySource::Person);
    assert_eq!(rule.task_id, None);
    tokio::time::sleep(Duration::from_millis(3)).await;

    let note: TeamMemoryLine = client
        .call(
            teams::MEMORY_ADD,
            &TeamMemoryAddParams {
                team: "alpha".to_owned(),
                bot: Some("mara".to_owned()),
                text: "Mara checks the tables.".to_owned(),
                task: None,
            },
        )
        .await
        .expect("a line in a bot scope");
    assert_eq!(note.bot_id.as_deref(), Some(mara.id.as_str()));
    tokio::time::sleep(Duration::from_millis(3)).await;

    let from_task: TeamMemoryLine = client
        .call(
            teams::MEMORY_ADD,
            &TeamMemoryAddParams {
                team: "alpha".to_owned(),
                bot: Some("mara".to_owned()),
                text: "The config keeps its keys.".to_owned(),
                task: Some(task.id.clone()),
            },
        )
        .await
        .expect("a line written by a task");
    assert_eq!(from_task.source, MemorySource::Task);
    assert_eq!(from_task.task_id.as_deref(), Some(task.id.as_str()));

    let everything: TeamMemoryList = client
        .call(
            teams::MEMORY_LIST,
            &TeamMemoryListParams {
                team: "alpha".to_owned(),
                bot: None,
            },
        )
        .await
        .expect("team.memory_list");
    assert_eq!(
        memory_ids(&everything.lines),
        [from_task.id.clone(), note.id.clone(), rule.id.clone()]
    );

    let mara_only: TeamMemoryList = client
        .call(
            teams::MEMORY_LIST,
            &TeamMemoryListParams {
                team: "alpha".to_owned(),
                bot: Some("mara".to_owned()),
            },
        )
        .await
        .expect("a bot lines");
    assert_eq!(
        memory_ids(&mara_only.lines),
        [from_task.id.clone(), note.id.clone()]
    );

    let removed: TeamMemoryLine = client
        .call(
            teams::MEMORY_REMOVE,
            &TeamMemoryRemoveParams {
                team: "alpha".to_owned(),
                line: rule.id.clone(),
            },
        )
        .await
        .expect("team.memory_rm");
    assert_eq!(removed, rule);
    let left: TeamMemoryList = client
        .call(
            teams::MEMORY_LIST,
            &TeamMemoryListParams {
                team: "alpha".to_owned(),
                bot: None,
            },
        )
        .await
        .expect("team.memory_list");
    assert_eq!(
        memory_ids(&left.lines),
        [from_task.id.clone(), note.id.clone()]
    );
    stop(running).await;
}

#[tokio::test]
async fn team_memory_text_is_bounded_and_unknown_selectors_are_refused_with_typed_codes() {
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
    client
        .call::<_, Bot>(teams::BOT_ADD, &bot_params("alpha", "mara", false))
        .await
        .expect("team.bot_add");
    let beta_task: Task = client
        .call(
            tasks::CREATE,
            &TaskCreateParams {
                team: "beta".to_owned(),
                title: "Parse the config".to_owned(),
                body: None,
                bot: None,
                project: None,
                origin: None,
            },
        )
        .await
        .expect("beta task");

    let longest = "a".repeat(500);
    let accepted: TeamMemoryLine = client
        .call(teams::MEMORY_ADD, &memory_add("alpha", &longest))
        .await
        .expect("500 characters");
    assert_eq!(accepted.text.chars().count(), 500);
    let accents = "é".repeat(500);
    client
        .call::<_, TeamMemoryLine>(teams::MEMORY_ADD, &memory_add("alpha", &accents))
        .await
        .expect("500 scalar values, whatever their bytes");

    let too_long = client
        .call::<_, TeamMemoryLine>(teams::MEMORY_ADD, &memory_add("alpha", &"a".repeat(501)))
        .await
        .expect_err("501 characters");
    assert_eq!(remote_code(&too_long), codes::REQUEST_INVALID_PARAMS);
    let blank = client
        .call::<_, TeamMemoryLine>(teams::MEMORY_ADD, &memory_add("alpha", "   "))
        .await
        .expect_err("a blank text");
    assert_eq!(remote_code(&blank), codes::REQUEST_INVALID_PARAMS);

    let unknown_team = client
        .call::<_, TeamMemoryLine>(teams::MEMORY_ADD, &memory_add("nowhere", "x"))
        .await
        .expect_err("an unknown team");
    assert_eq!(remote_code(&unknown_team), codes::TEAM_NOT_FOUND);
    let unknown_bot = client
        .call::<_, TeamMemoryLine>(
            teams::MEMORY_ADD,
            &TeamMemoryAddParams {
                team: "alpha".to_owned(),
                bot: Some("nobody".to_owned()),
                text: "x".to_owned(),
                task: None,
            },
        )
        .await
        .expect_err("an unknown bot");
    assert_eq!(remote_code(&unknown_bot), codes::BOT_NOT_FOUND);
    let other_teams_task = client
        .call::<_, TeamMemoryLine>(
            teams::MEMORY_ADD,
            &TeamMemoryAddParams {
                team: "alpha".to_owned(),
                bot: None,
                text: "x".to_owned(),
                task: Some(beta_task.id.clone()),
            },
        )
        .await
        .expect_err("a task of another team");
    assert_eq!(remote_code(&other_teams_task), codes::TASK_NOT_FOUND);

    let unknown_line = client
        .call::<_, TeamMemoryLine>(
            teams::MEMORY_REMOVE,
            &TeamMemoryRemoveParams {
                team: "alpha".to_owned(),
                line: "missing".to_owned(),
            },
        )
        .await
        .expect_err("an unknown line");
    assert_eq!(remote_code(&unknown_line), codes::TEAM_MEMORY_NOT_FOUND);
    let through_beta = client
        .call::<_, TeamMemoryLine>(
            teams::MEMORY_REMOVE,
            &TeamMemoryRemoveParams {
                team: "beta".to_owned(),
                line: accepted.id.clone(),
            },
        )
        .await
        .expect_err("a line of another team");
    assert_eq!(remote_code(&through_beta), codes::TEAM_MEMORY_NOT_FOUND);
    stop(running).await;
}

#[tokio::test]
async fn team_memory_changes_publish_team_changed_with_the_team_id() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let mut events = client.subscribe_events().await.expect("subscribe");
    let alpha: Team = client
        .call(teams::CREATE, &create_params("alpha"))
        .await
        .expect("alpha");
    assert_eq!(next_event(&mut events).await.id, alpha.id);

    let line: TeamMemoryLine = client
        .call(teams::MEMORY_ADD, &memory_add("alpha", "Keep it small."))
        .await
        .expect("team.memory_add");
    assert_eq!(
        next_event(&mut events).await,
        ChangeEvent {
            topic: TOPIC_TEAM_CHANGED.to_owned(),
            id: alpha.id.clone()
        }
    );

    client
        .call::<_, TeamMemoryLine>(
            teams::MEMORY_REMOVE,
            &TeamMemoryRemoveParams {
                team: "alpha".to_owned(),
                line: line.id.clone(),
            },
        )
        .await
        .expect("team.memory_rm");
    assert_eq!(
        next_event(&mut events).await,
        ChangeEvent {
            topic: TOPIC_TEAM_CHANGED.to_owned(),
            id: alpha.id.clone()
        }
    );
    stop(running).await;
}

/// A git repository with one commit at `dir`, so `project.add` registers it as a git project.
fn git_repo(dir: &Path) {
    std::fs::create_dir_all(dir).expect("create repository dir");
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("README.md"), "hello\n").expect("write readme");
    git(dir, &["add", "README.md"]);
    git(dir, &["commit", "-q", "-m", "initial"]);
}

/// Registers `path` as a Workspaces project: git, unless `kind` says folder.
async fn register_project(client: &Client, path: &Path, kind: Option<ProjectKind>) -> Project {
    let params = ProjectAddParams {
        icon: None,
        path: path.to_str().expect("utf-8 path").to_owned(),
        name: None,
        kind,
        workspaces_root: None,
    };
    client
        .call(projects::ADD, &params)
        .await
        .expect("project.add")
}

/// Adds `name` to team "alpha", linked to the Workspaces project `source` when given.
fn project_add(name: &str, source: Option<&str>) -> TeamProjectAddParams {
    TeamProjectAddParams {
        team: "alpha".to_owned(),
        name: name.to_owned(),
        goal: None,
        brief: None,
        source_project: source.map(str::to_owned),
        code_source: None,
    }
}

/// Changes the source of `project` in team "alpha": `Some("")` clears it, `None` leaves it as it is.
fn project_update_source(project: &str, source: Option<&str>) -> TeamProjectUpdateParams {
    TeamProjectUpdateParams {
        team: "alpha".to_owned(),
        project: project.to_owned(),
        name: None,
        goal: None,
        brief: None,
        source_project: source.map(str::to_owned),
        code_source: None,
    }
}

fn alpha_team() -> TeamRef {
    TeamRef {
        team: "alpha".to_owned(),
    }
}

#[tokio::test]
async fn a_team_project_links_a_git_or_a_folder_workspaces_project_as_its_source() {
    let home = temp_home();
    let repo = home.path().join("repo");
    let folder = home.path().join("notes");
    git_repo(&repo);
    std::fs::create_dir_all(&folder).expect("create folder");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &workspaces_team("alpha"))
        .await
        .expect("alpha");
    let repo_project = register_project(&client, &repo, None).await;
    let folder_project = register_project(&client, &folder, Some(ProjectKind::Folder)).await;

    let parser: TeamProject = client
        .call(
            teams::PROJECT_ADD,
            &project_add("parser", Some(&repo_project.name)),
        )
        .await
        .expect("linked by name");
    assert_eq!(
        parser.source_project.as_deref(),
        Some(repo_project.id.as_str())
    );
    assert_eq!(parser.source_project_name.as_deref(), Some("repo"));
    assert_eq!(parser.source_kind, Some(ProjectKind::Git));

    let folder_path = folder.to_str().expect("utf-8 path");
    let lexer: TeamProject = client
        .call(teams::PROJECT_ADD, &project_add("lexer", Some(folder_path)))
        .await
        .expect("linked by path");
    assert_eq!(
        lexer.source_project.as_deref(),
        Some(folder_project.id.as_str())
    );
    assert_eq!(lexer.source_kind, Some(ProjectKind::Folder));

    let listed: TeamProjectList = client
        .call(teams::PROJECT_LIST, &alpha_team())
        .await
        .expect("team.project_list");
    let listed_parser = listed
        .projects
        .iter()
        .find(|project| project.id == parser.id)
        .expect("parser is listed");
    assert_eq!(listed_parser.source_project_name.as_deref(), Some("repo"));
    assert_eq!(listed_parser.source_kind, Some(ProjectKind::Git));
    stop(running).await;
}

#[tokio::test]
async fn updating_the_source_moves_it_and_an_empty_one_clears_it() {
    let home = temp_home();
    let repo = home.path().join("repo");
    let folder = home.path().join("notes");
    git_repo(&repo);
    std::fs::create_dir_all(&folder).expect("create folder");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &workspaces_team("alpha"))
        .await
        .expect("alpha");
    let repo_project = register_project(&client, &repo, None).await;
    let folder_project = register_project(&client, &folder, Some(ProjectKind::Folder)).await;
    let parser: TeamProject = client
        .call(
            teams::PROJECT_ADD,
            &project_add("parser", Some(&repo_project.id)),
        )
        .await
        .expect("linked by id");

    let moved: TeamProject = client
        .call(
            teams::PROJECT_UPDATE,
            &project_update_source("parser", Some(&folder_project.id)),
        )
        .await
        .expect("moved to the folder");
    assert_eq!(
        moved.source_project.as_deref(),
        Some(folder_project.id.as_str())
    );
    assert_eq!(moved.source_project_name.as_deref(), Some("notes"));
    assert_eq!(moved.source_kind, Some(ProjectKind::Folder));

    let cleared: TeamProject = client
        .call(
            teams::PROJECT_UPDATE,
            &project_update_source("parser", Some("")),
        )
        .await
        .expect("an empty source clears the link");
    assert_eq!(cleared.id, parser.id);
    assert_eq!(cleared.name, "parser");
    assert_eq!(cleared.source_project, None);
    assert_eq!(cleared.source_project_name, None);
    assert_eq!(cleared.source_kind, None);

    let unknown = client
        .call::<_, TeamProject>(
            teams::PROJECT_UPDATE,
            &project_update_source("parser", Some("nothing")),
        )
        .await
        .expect_err("an unknown source project");
    assert_eq!(remote_code(&unknown), codes::PROJECT_NOT_FOUND);
    let unknown_on_add = client
        .call::<_, TeamProject>(teams::PROJECT_ADD, &project_add("lexer", Some("nothing")))
        .await
        .expect_err("an unknown source project on add");
    assert_eq!(remote_code(&unknown_on_add), codes::PROJECT_NOT_FOUND);
    let listed: TeamProjectList = client
        .call(teams::PROJECT_LIST, &alpha_team())
        .await
        .expect("team.project_list");
    assert_eq!(listed.projects.len(), 1, "a refused add creates no project");
    stop(running).await;
}

#[tokio::test]
async fn removing_the_workspaces_project_clears_the_link_and_keeps_the_team_project() {
    let home = temp_home();
    let folder = home.path().join("notes");
    std::fs::create_dir_all(&folder).expect("create folder");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &workspaces_team("alpha"))
        .await
        .expect("alpha");
    let folder_project = register_project(&client, &folder, Some(ProjectKind::Folder)).await;
    let parser: TeamProject = client
        .call(
            teams::PROJECT_ADD,
            &project_add("parser", Some(&folder_project.id)),
        )
        .await
        .expect("linked");

    let removed: Project = client
        .call(
            projects::REMOVE,
            &ProjectRef {
                project: folder_project.id.clone(),
            },
        )
        .await
        .expect("project.remove");
    assert_eq!(removed.id, folder_project.id);

    let listed: TeamProjectList = client
        .call(teams::PROJECT_LIST, &alpha_team())
        .await
        .expect("team.project_list");
    assert_eq!(listed.projects.len(), 1);
    let kept = &listed.projects[0];
    assert_eq!(kept.id, parser.id);
    assert_eq!(kept.name, "parser");
    assert_eq!(kept.source_project, None);
    assert_eq!(kept.source_project_name, None);
    assert_eq!(kept.source_kind, None);
    stop(running).await;
}

fn skills_update(skills: Option<Vec<String>>) -> BotUpdateParams {
    BotUpdateParams {
        team: "alpha".to_owned(),
        bot: "mara".to_owned(),
        name: None,
        job: None,
        kind: None,
        model: None,
        effort: None,
        lead: None,
        skills,
    }
}

fn skill_list(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[tokio::test]
async fn a_bot_keeps_its_skills_in_order_and_an_update_replaces_or_clears_them() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &create_params("alpha"))
        .await
        .expect("team.create");

    let plain: Bot = client
        .call(teams::BOT_ADD, &bot_params("alpha", "bruno", false))
        .await
        .expect("a bot without skills");
    assert!(plain.skills.is_empty());

    let added: Bot = client
        .call(
            teams::BOT_ADD,
            &BotAddParams {
                skills: skill_list(&[" review ", "deploy", "", "review"]),
                ..bot_params("alpha", "mara", false)
            },
        )
        .await
        .expect("a bot with skills");
    assert_eq!(added.skills, skill_list(&["review", "deploy"]));

    let renamed: Bot = client
        .call(
            teams::BOT_UPDATE,
            &BotUpdateParams {
                job: Some("Reviews".to_owned()),
                ..skills_update(None)
            },
        )
        .await
        .expect("an update without skills");
    assert_eq!(renamed.skills, skill_list(&["review", "deploy"]));

    let replaced: Bot = client
        .call(
            teams::BOT_UPDATE,
            &skills_update(Some(skill_list(&["notes"]))),
        )
        .await
        .expect("an update that replaces the skills");
    assert_eq!(replaced.skills, skill_list(&["notes"]));
    let listed: BotList = client
        .call(teams::BOT_LIST, &alpha_team())
        .await
        .expect("team.bot_list");
    let skills: Vec<Vec<String>> = listed.bots.into_iter().map(|bot| bot.skills).collect();
    assert_eq!(skills, vec![Vec::new(), skill_list(&["notes"])]);

    let too_many = client
        .call::<_, Bot>(
            teams::BOT_UPDATE,
            &skills_update(Some(
                (0..=teams::MAX_BOT_SKILLS)
                    .map(|n| format!("skill-{n}"))
                    .collect(),
            )),
        )
        .await
        .expect_err("more skills than a bot takes");
    assert_eq!(remote_code(&too_many), codes::REQUEST_INVALID_PARAMS);

    let cleared: Bot = client
        .call(teams::BOT_UPDATE, &skills_update(Some(Vec::new())))
        .await
        .expect("an empty list clears the skills");
    assert!(cleared.skills.is_empty());
    stop(running).await;
}

fn workspaces_switch(on: bool) -> TeamUpdateParams {
    TeamUpdateParams {
        team: "alpha".to_owned(),
        name: None,
        purpose: None,
        character: None,
        access: None,
        workspaces: Some(on),
    }
}

#[tokio::test]
async fn a_team_with_workspaces_off_cannot_connect_a_project_to_code_but_may_unlink_and_edit_it() {
    let home = temp_home();
    let folder = home.path().join("notes");
    std::fs::create_dir_all(&folder).expect("create folder");
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &create_params("alpha"))
        .await
        .expect("alpha, Workspaces off");
    let folder_project = register_project(&client, &folder, Some(ProjectKind::Folder)).await;

    let linked_on_add = client
        .call::<_, TeamProject>(
            teams::PROJECT_ADD,
            &project_add("parser", Some(&folder_project.id)),
        )
        .await
        .expect_err("a link needs Workspaces");
    assert_eq!(remote_code(&linked_on_add), codes::TEAM_WORKSPACES_OFF);
    let source_on_add = client
        .call::<_, TeamProject>(
            teams::PROJECT_ADD,
            &TeamProjectAddParams {
                code_source: Some(CodeSource::ExistingRepository),
                ..project_add("parser", None)
            },
        )
        .await
        .expect_err("a code source needs Workspaces");
    assert_eq!(remote_code(&source_on_add), codes::TEAM_WORKSPACES_OFF);

    client
        .call::<_, TeamProject>(teams::PROJECT_ADD, &project_add("parser", None))
        .await
        .expect("a project without code");
    let linked_on_update = client
        .call::<_, TeamProject>(
            teams::PROJECT_UPDATE,
            &project_update_source("parser", Some(&folder_project.id)),
        )
        .await
        .expect_err("a link needs Workspaces on update too");
    assert_eq!(remote_code(&linked_on_update), codes::TEAM_WORKSPACES_OFF);

    client
        .call::<_, Team>(teams::UPDATE, &workspaces_switch(true))
        .await
        .expect("Workspaces on");
    let linked: TeamProject = client
        .call(
            teams::PROJECT_UPDATE,
            &project_update_source("parser", Some(&folder_project.id)),
        )
        .await
        .expect("linked once Workspaces is on");
    assert_eq!(
        linked.source_project.as_deref(),
        Some(folder_project.id.as_str())
    );

    client
        .call::<_, Team>(teams::UPDATE, &workspaces_switch(false))
        .await
        .expect("Workspaces off again");
    let renamed: TeamProject = client
        .call(
            teams::PROJECT_UPDATE,
            &TeamProjectUpdateParams {
                goal: Some("Parse it".to_owned()),
                ..project_update_source("parser", None)
            },
        )
        .await
        .expect("edits other than code stay open");
    assert_eq!(renamed.goal, "Parse it");
    let unlinked: TeamProject = client
        .call(
            teams::PROJECT_UPDATE,
            &project_update_source("parser", Some("")),
        )
        .await
        .expect("clearing a link stays open");
    assert_eq!(unlinked.source_project, None);
    stop(running).await;
}
