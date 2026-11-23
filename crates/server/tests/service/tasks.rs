//! Tasks and the Board over the wire: create, list, show, update, done, reopen and remove, the Board's columns, the links a
//! bot or project removal leaves, and the events. Sessions cannot be started yet, so no task here has a running session.

use std::time::Duration;

use massorca_client::Subscription;
use massorca_core_store::{Store, TaskOutcome};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_TASK_CHANGED, TOPIC_TEAM_CHANGED};
use massorca_protocol::methods::tasks::{
    self, CardState, Column, Task, TaskCreateParams, TaskList, TaskListParams, TaskOrigin, TaskRef,
    TaskUpdateParams,
};
use massorca_protocol::methods::teams::{
    self, Board, BoardParams, Bot, BotAddParams, BotRemoveParams, Team, TeamCreateParams,
    TeamProject, TeamProjectAddParams, TeamProjectRemoveParams, TeamRef, TeamRemoveParams,
};
use tokio::time::{sleep, timeout};

use super::common::{connect, profile, remote_code, start, stop, temp_home};

fn team_params(name: &str) -> TeamCreateParams {
    TeamCreateParams {
        name: name.to_owned(),
        purpose: None,
        character: None,
        access: None,
        workspaces: None,
    }
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

fn task_params(team: &str, title: &str) -> TaskCreateParams {
    TaskCreateParams {
        team: team.to_owned(),
        title: title.to_owned(),
        body: None,
        bot: None,
        project: None,
        origin: None,
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
async fn a_task_is_created_in_the_open_column_and_listed_shown_and_updated() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let alpha: Team = client
        .call(teams::CREATE, &team_params("alpha"))
        .await
        .expect("team.create");
    let mara: Bot = client
        .call(teams::BOT_ADD, &bot_params("alpha", "mara", true))
        .await
        .expect("team.bot_add");
    let parser: TeamProject = client
        .call(
            teams::PROJECT_ADD,
            &TeamProjectAddParams {
                source_project: None,
                team: "alpha".to_owned(),
                name: "parser".to_owned(),
                goal: None,
                brief: None,
            },
        )
        .await
        .expect("team.project_add");

    let task: Task = client
        .call(
            tasks::CREATE,
            &TaskCreateParams {
                body: Some("First the magic bytes.".to_owned()),
                bot: Some("mara".to_owned()),
                project: Some("parser".to_owned()),
                ..task_params("alpha", "parse the header")
            },
        )
        .await
        .expect("task.create");
    assert_eq!(task.team_id, alpha.id);
    assert_eq!(task.bot_id.as_deref(), Some(mara.id.as_str()));
    assert_eq!(task.project_id.as_deref(), Some(parser.id.as_str()));
    assert_eq!(task.origin, TaskOrigin::Cli);
    assert_eq!(
        (task.column, task.state, task.outcome),
        (Column::Open, CardState::NotStarted, None)
    );

    let shown: Task = client
        .call(
            tasks::SHOW,
            &TaskRef {
                task: task.id.clone(),
            },
        )
        .await
        .expect("task.show");
    assert_eq!(shown, task);

    let open: TaskList = client
        .call(
            tasks::LIST,
            &TaskListParams {
                team: Some("alpha".to_owned()),
                column: Some(Column::Open),
                ..TaskListParams::default()
            },
        )
        .await
        .expect("task.list by column");
    assert_eq!(open.tasks, vec![task.clone()]);
    let by_bot: TaskList = client
        .call(
            tasks::LIST,
            &TaskListParams {
                team: Some("alpha".to_owned()),
                bot: Some("mara".to_owned()),
                ..TaskListParams::default()
            },
        )
        .await
        .expect("task.list by bot");
    assert_eq!(by_bot.tasks.len(), 1);
    let done: TaskList = client
        .call(
            tasks::LIST,
            &TaskListParams {
                column: Some(Column::Done),
                ..TaskListParams::default()
            },
        )
        .await
        .expect("task.list done");
    assert!(done.tasks.is_empty());

    let updated: Task = client
        .call(
            tasks::UPDATE,
            &TaskUpdateParams {
                task: task.id.clone(),
                title: Some("parse the body".to_owned()),
                body: None,
                bot: Some(String::new()),
                project: None,
            },
        )
        .await
        .expect("task.update");
    assert_eq!(updated.title, "parse the body");
    assert_eq!(updated.bot_id, None);
    assert_eq!(updated.project_id.as_deref(), Some(parser.id.as_str()));
    stop(running).await;
}

#[tokio::test]
async fn the_board_has_four_columns_in_order_and_each_card_sits_in_its_column() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &team_params("alpha"))
        .await
        .expect("team.create");
    client
        .call::<_, Bot>(teams::BOT_ADD, &bot_params("alpha", "mara", false))
        .await
        .expect("team.bot_add");
    client
        .call::<_, TeamProject>(
            teams::PROJECT_ADD,
            &TeamProjectAddParams {
                source_project: None,
                team: "alpha".to_owned(),
                name: "parser".to_owned(),
                goal: None,
                brief: None,
            },
        )
        .await
        .expect("team.project_add");
    let first: Task = client
        .call(
            tasks::CREATE,
            &TaskCreateParams {
                bot: Some("mara".to_owned()),
                project: Some("parser".to_owned()),
                ..task_params("alpha", "first")
            },
        )
        .await
        .expect("first task");
    // Task ids sort by creation time only across milliseconds, so the second task waits one.
    sleep(Duration::from_millis(2)).await;
    let second: Task = client
        .call(tasks::CREATE, &task_params("alpha", "second"))
        .await
        .expect("second task");

    let board: Board = client
        .call(
            teams::BOARD,
            &BoardParams {
                team: "alpha".to_owned(),
                project: None,
            },
        )
        .await
        .expect("team.board");
    let columns: Vec<Column> = board.columns.iter().map(|column| column.column).collect();
    assert_eq!(columns, Column::ALL);
    let open_titles: Vec<&str> = board.columns[0]
        .cards
        .iter()
        .map(|card| card.title.as_str())
        .collect();
    assert_eq!(open_titles, ["first", "second"]);
    let card = &board.columns[0].cards[0];
    assert_eq!(card.id, first.id);
    assert_eq!(card.bot.as_deref(), Some("mara"));
    assert_eq!(card.state, CardState::NotStarted);
    assert!(card.at_ms > 0);
    assert!(
        board.columns[1..]
            .iter()
            .all(|column| column.cards.is_empty())
    );
    assert!(board.project_id.is_none());

    let parser_board: Board = client
        .call(
            teams::BOARD,
            &BoardParams {
                team: "alpha".to_owned(),
                project: Some("parser".to_owned()),
            },
        )
        .await
        .expect("team.board of a project");
    assert_eq!(parser_board.columns[0].cards.len(), 1);
    assert_eq!(parser_board.columns[0].cards[0].id, first.id);
    assert_ne!(parser_board.columns[0].cards[0].id, second.id);

    let unknown = client
        .call::<_, Board>(
            teams::BOARD,
            &BoardParams {
                team: "alpha".to_owned(),
                project: Some("nothing".to_owned()),
            },
        )
        .await
        .expect_err("an unknown project");
    assert_eq!(remote_code(&unknown), codes::TEAM_PROJECT_NOT_FOUND);
    stop(running).await;
}

#[tokio::test]
async fn done_and_reopen_move_a_task_between_open_and_done() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &team_params("alpha"))
        .await
        .expect("team.create");
    let task: Task = client
        .call(tasks::CREATE, &task_params("alpha", "ship it"))
        .await
        .expect("task.create");

    let done: Task = client
        .call(
            tasks::DONE,
            &TaskRef {
                task: task.id.clone(),
            },
        )
        .await
        .expect("task.done");
    assert_eq!(done.column, Column::Done);
    assert_eq!(done.state, CardState::Done);
    assert_eq!(done.outcome, Some(Outcome::Success));
    let board: Board = client
        .call(
            teams::BOARD,
            &BoardParams {
                team: "alpha".to_owned(),
                project: None,
            },
        )
        .await
        .expect("team.board");
    assert!(board.columns[0].cards.is_empty());
    assert_eq!(board.columns[3].cards[0].id, task.id);

    let reopened: Task = client
        .call(
            tasks::REOPEN,
            &TaskRef {
                task: task.id.clone(),
            },
        )
        .await
        .expect("task.reopen");
    assert_eq!(reopened.column, Column::Open);
    assert_eq!(reopened.outcome, None);
    assert_eq!(reopened.error, None);
    stop(running).await;
}

#[tokio::test]
async fn a_bad_task_request_is_refused_with_typed_codes() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &team_params("alpha"))
        .await
        .expect("alpha");
    client
        .call::<_, Team>(teams::CREATE, &team_params("beta"))
        .await
        .expect("beta");
    client
        .call::<_, Bot>(teams::BOT_ADD, &bot_params("beta", "bruno", false))
        .await
        .expect("bruno in beta");

    let no_team = client
        .call::<_, Task>(tasks::CREATE, &task_params("nothing", "orphan"))
        .await
        .expect_err("an unknown team");
    assert_eq!(remote_code(&no_team), codes::TEAM_NOT_FOUND);

    let other_teams_bot = client
        .call::<_, Task>(
            tasks::CREATE,
            &TaskCreateParams {
                bot: Some("bruno".to_owned()),
                ..task_params("alpha", "wrong bot")
            },
        )
        .await
        .expect_err("a bot of another team");
    assert_eq!(remote_code(&other_teams_bot), codes::BOT_NOT_FOUND);

    let no_project = client
        .call::<_, Task>(
            tasks::CREATE,
            &TaskCreateParams {
                project: Some("nothing".to_owned()),
                ..task_params("alpha", "no project")
            },
        )
        .await
        .expect_err("an unknown project");
    assert_eq!(remote_code(&no_project), codes::TEAM_PROJECT_NOT_FOUND);

    let blank = client
        .call::<_, Task>(tasks::CREATE, &task_params("alpha", "   "))
        .await
        .expect_err("a blank title");
    assert_eq!(remote_code(&blank), codes::REQUEST_INVALID_PARAMS);

    let missing = client
        .call::<_, Task>(
            tasks::SHOW,
            &TaskRef {
                task: "missing".to_owned(),
            },
        )
        .await
        .expect_err("an unknown task");
    assert_eq!(remote_code(&missing), codes::TASK_NOT_FOUND);

    let project_without_team = client
        .call::<_, TaskList>(
            tasks::LIST,
            &TaskListParams {
                project: Some("parser".to_owned()),
                ..TaskListParams::default()
            },
        )
        .await
        .expect_err("a project filter needs its team");
    assert_eq!(
        remote_code(&project_without_team),
        codes::REQUEST_INVALID_PARAMS
    );
    stop(running).await;
}

#[tokio::test]
async fn task_changes_publish_task_changed_and_a_removed_task_is_gone() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let mut events = client.subscribe_events().await.expect("subscribe");
    client
        .call::<_, Team>(teams::CREATE, &team_params("alpha"))
        .await
        .expect("team.create");
    assert_eq!(next_event(&mut events).await.topic, TOPIC_TEAM_CHANGED);

    let task: Task = client
        .call(tasks::CREATE, &task_params("alpha", "ship it"))
        .await
        .expect("task.create");
    assert_eq!(
        next_event(&mut events).await,
        ChangeEvent {
            topic: TOPIC_TASK_CHANGED.to_owned(),
            id: task.id.clone()
        }
    );
    client
        .call::<_, Task>(
            tasks::DONE,
            &TaskRef {
                task: task.id.clone(),
            },
        )
        .await
        .expect("task.done");
    assert_eq!(next_event(&mut events).await.id, task.id);
    client
        .call::<_, Task>(
            tasks::REMOVE,
            &TaskRef {
                task: task.id.clone(),
            },
        )
        .await
        .expect("task.rm");
    assert_eq!(next_event(&mut events).await.topic, TOPIC_TASK_CHANGED);
    let gone = client
        .call::<_, Task>(
            tasks::SHOW,
            &TaskRef {
                task: task.id.clone(),
            },
        )
        .await
        .expect_err("a removed task");
    assert_eq!(remote_code(&gone), codes::TASK_NOT_FOUND);
    stop(running).await;
}

#[tokio::test]
async fn removing_a_bot_with_open_tasks_needs_unassign_and_its_tasks_lose_it() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &team_params("alpha"))
        .await
        .expect("team.create");
    let mara: Bot = client
        .call(teams::BOT_ADD, &bot_params("alpha", "mara", false))
        .await
        .expect("team.bot_add");
    let open: Task = client
        .call(
            tasks::CREATE,
            &TaskCreateParams {
                bot: Some("mara".to_owned()),
                ..task_params("alpha", "open work")
            },
        )
        .await
        .expect("open task");
    let finished: Task = client
        .call(
            tasks::CREATE,
            &TaskCreateParams {
                bot: Some("mara".to_owned()),
                ..task_params("alpha", "finished work")
            },
        )
        .await
        .expect("finished task");
    client
        .call::<_, Task>(
            tasks::DONE,
            &TaskRef {
                task: finished.id.clone(),
            },
        )
        .await
        .expect("task.done");

    let refused = client
        .call::<_, Bot>(
            teams::BOT_REMOVE,
            &BotRemoveParams {
                team: "alpha".to_owned(),
                bot: "mara".to_owned(),
                unassign: false,
            },
        )
        .await
        .expect_err("a bot with an open task");
    assert_eq!(remote_code(&refused), codes::BOT_HAS_TASKS);

    let mut events = client.subscribe_events().await.expect("subscribe");
    let removed: Bot = client
        .call(
            teams::BOT_REMOVE,
            &BotRemoveParams {
                team: "alpha".to_owned(),
                bot: "mara".to_owned(),
                unassign: true,
            },
        )
        .await
        .expect("team.bot_rm with unassign");
    assert_eq!(removed.id, mara.id);
    let mut changed = Vec::new();
    for _ in 0..3 {
        changed.push(next_event(&mut events).await);
    }
    assert!(
        changed
            .iter()
            .any(|event| event.topic == TOPIC_TEAM_CHANGED)
    );
    for task in [&open, &finished] {
        assert!(
            changed
                .iter()
                .any(|event| event.topic == TOPIC_TASK_CHANGED && event.id == task.id),
            "task.changed for {}",
            task.id
        );
        let after: Task = client
            .call(
                tasks::SHOW,
                &TaskRef {
                    task: task.id.clone(),
                },
            )
            .await
            .expect("the task is kept");
        assert_eq!(after.bot_id, None);
    }
    stop(running).await;
}

#[tokio::test]
async fn removing_a_project_leaves_its_tasks_on_the_board_without_it() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &team_params("alpha"))
        .await
        .expect("team.create");
    client
        .call::<_, TeamProject>(
            teams::PROJECT_ADD,
            &TeamProjectAddParams {
                source_project: None,
                team: "alpha".to_owned(),
                name: "parser".to_owned(),
                goal: None,
                brief: None,
            },
        )
        .await
        .expect("team.project_add");
    let task: Task = client
        .call(
            tasks::CREATE,
            &TaskCreateParams {
                project: Some("parser".to_owned()),
                ..task_params("alpha", "parse")
            },
        )
        .await
        .expect("task.create");

    client
        .call::<_, TeamProject>(
            teams::PROJECT_REMOVE,
            &TeamProjectRemoveParams {
                team: "alpha".to_owned(),
                project: "parser".to_owned(),
            },
        )
        .await
        .expect("team.project_rm");

    let after: Task = client
        .call(
            tasks::SHOW,
            &TaskRef {
                task: task.id.clone(),
            },
        )
        .await
        .expect("the task is kept");
    assert_eq!(after.project_id, None);
    assert_eq!(after.column, Column::Open);
    stop(running).await;
}

#[tokio::test]
async fn removing_a_team_removes_its_tasks_and_publishes_task_changed_for_each() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let alpha: Team = client
        .call(teams::CREATE, &team_params("alpha"))
        .await
        .expect("team.create");
    let first: Task = client
        .call(tasks::CREATE, &task_params("alpha", "first"))
        .await
        .expect("first task");
    let second: Task = client
        .call(tasks::CREATE, &task_params("alpha", "second"))
        .await
        .expect("second task");
    let mut events = client.subscribe_events().await.expect("subscribe");

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

    let mut removed_tasks = Vec::new();
    for _ in 0..3 {
        let event = next_event(&mut events).await;
        if event.topic == TOPIC_TASK_CHANGED {
            removed_tasks.push(event.id);
        }
    }
    removed_tasks.sort();
    let mut expected = vec![first.id.clone(), second.id.clone()];
    expected.sort();
    assert_eq!(removed_tasks, expected);
    let gone = client
        .call::<_, Team>(
            teams::SHOW,
            &TeamRef {
                team: alpha.id.clone(),
            },
        )
        .await
        .expect_err("the team is gone");
    assert_eq!(remote_code(&gone), codes::TEAM_NOT_FOUND);
    let task_gone = client
        .call::<_, Task>(
            tasks::SHOW,
            &TaskRef {
                task: first.id.clone(),
            },
        )
        .await
        .expect_err("its tasks are gone");
    assert_eq!(remote_code(&task_gone), codes::TASK_NOT_FOUND);
    stop(running).await;
}

#[tokio::test]
async fn reopening_a_task_unlinks_its_session_so_it_can_start_again() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .call::<_, Team>(teams::CREATE, &team_params("alpha"))
        .await
        .expect("team.create");
    let task: Task = client
        .call(tasks::CREATE, &task_params("alpha", "ship it"))
        .await
        .expect("task.create");

    // A session linked the way task.start will link one, to a session the agents domain has forgotten.
    let store = Store::open(profile(home.path(), "default").store_path()).expect("the store opens");
    assert!(
        store
            .link_task_session(&task.id, "forgotten-session", 2)
            .expect("the link is stored")
    );
    assert!(
        store
            .set_task_outcome(&task.id, TaskOutcome::Failure, Some("forgotten-session"), 3)
            .expect("the outcome is stored")
    );
    let stuck: Task = client
        .call(
            tasks::SHOW,
            &TaskRef {
                task: task.id.clone(),
            },
        )
        .await
        .expect("task.show");
    assert_eq!(
        (stuck.column, stuck.state),
        (Column::Waiting, CardState::Stuck)
    );

    let reopened: Task = client
        .call(
            tasks::REOPEN,
            &TaskRef {
                task: task.id.clone(),
            },
        )
        .await
        .expect("task.reopen");
    assert_eq!(reopened.session_id, None);
    assert_eq!(reopened.outcome, None);
    assert_eq!(
        (reopened.column, reopened.state),
        (Column::Open, CardState::NotStarted)
    );
    stop(running).await;
}
