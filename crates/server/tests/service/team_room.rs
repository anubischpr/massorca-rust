//! The Conference room over the wire: a person's line that names a day and a time becomes a one-time routine of its
//! bot, with the bot's answer about it (decision 0121), and a line with start off waits in Todo. None of these lines
//! starts an agent; a refused routine, which becomes a task, is covered by the CLI tests with the fake agent. A line
//! no bot can be picked for gets a choice card, answered once. A reply to a person's line stays a task that keeps the
//! quoted line (decision 0122); a conversation, which starts a bot's reply run, is covered by the CLI tests.

use massorca_protocol::codes;
use massorca_protocol::methods::events::TOPIC_TEAM_CHANGED;
use massorca_protocol::methods::tasks::{Column, Task, TaskListParams};
use massorca_protocol::methods::team_room::{
    RoomAuthor, RoomChoiceKind, TeamRoomAnswerParams, TeamRoomListParams, TeamRoomPostParams,
};
use massorca_protocol::methods::teams::{BotAddParams, RoutineCadence, TeamCreateParams, TeamRef};

use super::common::{connect, remote_code, start, stop, temp_home};

fn post(text: &str, bot: Option<&str>) -> TeamRoomPostParams {
    TeamRoomPostParams {
        team: "alpha".to_owned(),
        text: text.to_owned(),
        bot: bot.map(str::to_owned),
        to: None,
        session: None,
        agent_token: None,
        start: None,
        reply_to: None,
        ask: None,
        route: None,
    }
}

async fn team_with_bots(client: &massorca_client::Client) {
    team_with(client, "alpha", &[("Mara", false), ("Bruno", true)]).await;
}

async fn team_with(client: &massorca_client::Client, team: &str, bots: &[(&str, bool)]) {
    client
        .team_create(&TeamCreateParams {
            name: team.to_owned(),
            purpose: None,
            character: None,
            access: None,
            workspaces: None,
        })
        .await
        .expect("team.create");
    for &(name, lead) in bots {
        client
            .team_bot_add(&BotAddParams {
                team: team.to_owned(),
                name: name.to_owned(),
                job: "Keeps the docs".to_owned(),
                kind: "claude".to_owned(),
                model: None,
                effort: None,
                lead,
                skills: Vec::new(),
            })
            .await
            .expect("team.bot_add");
    }
}

#[tokio::test]
async fn a_line_with_a_day_and_a_time_becomes_a_one_time_routine_of_its_bot() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    team_with_bots(&client).await;
    let mut events = client.subscribe_events().await.expect("events");

    let posted = client
        .team_room_post(&post(
            "@Mara back up the docs on 2099-01-02 at 9:30pm",
            None,
        ))
        .await
        .expect("team.room_post");
    let [person, answer] = posted.messages.as_slice() else {
        panic!("a line and its answer: {posted:?}");
    };
    assert_eq!(person.author, RoomAuthor::Person);
    assert_eq!(answer.bot_name.as_deref(), Some("Mara"));
    assert!(answer.task.is_none(), "no task is made");
    let routine = answer
        .routine
        .as_ref()
        .expect("the answer carries its routine");
    assert_eq!(routine.cadence, RoutineCadence::Once);
    assert_eq!(routine.run_date.as_deref(), Some("2099-01-02"));
    assert_eq!(routine.time, "21:30");
    assert_eq!(
        routine.prompt,
        "@Mara back up the docs on 2099-01-02 at 9:30pm"
    );
    assert_eq!(
        answer.text,
        format!(
            "Routine created: {}. It runs once on 2099-01-02 at 21:30.",
            routine.name
        )
    );

    let listed = client
        .team_routine_list(&TeamRef {
            team: "alpha".to_owned(),
        })
        .await
        .expect("team.routine_list");
    assert_eq!(listed.routines, vec![routine.clone()]);
    let room = client
        .team_room_list(&TeamRoomListParams {
            team: "alpha".to_owned(),
            limit: None,
        })
        .await
        .expect("team.room_list");
    assert_eq!(room.messages, posted.messages);

    let mut topics = Vec::new();
    while topics.len() < 2 {
        let event = tokio::time::timeout(
            super::common::REPLY_WAIT,
            events.next_as::<massorca_protocol::methods::events::ChangeEvent>(),
        )
        .await
        .expect("an event arrives")
        .expect("stream open")
        .expect("event decodes");
        topics.push(event.topic);
    }
    assert!(topics.iter().any(|topic| topic == TOPIC_TEAM_CHANGED));
    stop(running).await;
}

#[tokio::test]
async fn a_bot_line_is_never_scheduled() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    team_with_bots(&client).await;

    let as_bot = client
        .team_room_post(&post(
            "I will back up the docs tomorrow at 09:00",
            Some("Mara"),
        ))
        .await
        .expect("a person posts as a bot");
    assert_eq!(as_bot.messages.len(), 1, "a bot's line is only stored");
    assert!(as_bot.messages[0].routine.is_none());
    let routines = client
        .team_routine_list(&TeamRef {
            team: "alpha".to_owned(),
        })
        .await
        .expect("team.routine_list");
    assert!(routines.routines.is_empty());
    stop(running).await;
}

#[tokio::test]
async fn a_line_with_start_off_waits_in_todo_with_its_mention_left_out_of_the_title() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    team_with_bots(&client).await;

    let mut params = post("@Mara, parse the headers\nthen stop.", None);
    params.start = Some(false);
    let posted = client
        .team_room_post(&params)
        .await
        .expect("team.room_post");
    let [person, answer] = posted.messages.as_slice() else {
        panic!("a line and its answer: {posted:?}");
    };
    assert_eq!(person.text, "@Mara, parse the headers\nthen stop.");
    assert_eq!(answer.bot_name.as_deref(), Some("Mara"));
    assert_eq!(
        answer.text,
        "Added to the Board, not started: parse the headers"
    );
    let task = answer.task.as_ref().expect("the answer carries its task");
    assert_eq!(task.title, "parse the headers");
    assert_eq!(task.body, "@Mara, parse the headers\nthen stop.");
    assert_eq!(task.column, Column::Open, "it waits in Todo");
    assert_eq!(task.session_id, None, "no agent started");
    stop(running).await;
}

fn held(text: &str) -> TeamRoomPostParams {
    let mut params = post(text, None);
    params.start = Some(false);
    params
}

fn answer(message: &str, choice: &str) -> TeamRoomAnswerParams {
    TeamRoomAnswerParams {
        team: "alpha".to_owned(),
        message: message.to_owned(),
        choice: choice.to_owned(),
    }
}

async fn tasks_of(client: &massorca_client::Client, team: &str) -> Vec<Task> {
    client
        .task_list(&TaskListParams {
            team: Some(team.to_owned()),
            project: None,
            bot: None,
            column: None,
        })
        .await
        .expect("task.list")
        .tasks
}

#[tokio::test]
async fn a_line_no_bot_can_be_picked_for_gets_a_card_answered_once_into_todo() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    team_with(&client, "alpha", &[("Mara", false), ("Bruno", false)]).await;
    team_with(&client, "beta", &[]).await;

    let posted = client
        .team_room_post(&post("Add magic link sign-in", None))
        .await
        .expect("team.room_post");
    let [person, card] = posted.messages.as_slice() else {
        panic!("a line and its card: {posted:?}");
    };
    assert_eq!(person.author, RoomAuthor::Person);
    assert_eq!(
        (card.author, card.bot_name.as_deref()),
        (RoomAuthor::Bot, None)
    );
    let offered = card.card.as_ref().expect("the line is a card");
    assert_eq!(offered.request_id, person.id);
    let kinds: Vec<(RoomChoiceKind, &str)> = offered
        .options
        .iter()
        .map(|option| (option.kind, option.label.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            (RoomChoiceKind::Bot, "Mara"),
            (RoomChoiceKind::Bot, "Bruno"),
            (RoomChoiceKind::Team, "beta"),
            (RoomChoiceKind::Todo, "Keep in this team's Todo"),
        ]
    );
    assert!(
        tasks_of(&client, "alpha").await.is_empty(),
        "no task until it is answered"
    );

    let not_offered = client
        .team_room_answer(&answer(&card.id, "bot:nobody"))
        .await
        .expect_err("a choice the card does not offer");
    assert_eq!(remote_code(&not_offered), codes::REQUEST_INVALID_PARAMS);
    let not_a_card = client
        .team_room_answer(&answer(&person.id, "todo"))
        .await
        .expect_err("a person's line is no card");
    assert_eq!(remote_code(&not_a_card), codes::TEAM_ROOM_CARD_NOT_FOUND);

    let answered = client
        .team_room_answer(&answer(&card.id, "todo"))
        .await
        .expect("team.room_answer");
    assert_eq!(
        answered
            .card
            .card
            .as_ref()
            .and_then(|card| card.answer.as_deref()),
        Some("todo")
    );
    let [confirmation] = answered.messages.as_slice() else {
        panic!("one confirmation: {answered:?}");
    };
    assert_eq!(
        confirmation.text,
        "Kept in this team's Todo: Add magic link sign-in"
    );
    let task = confirmation
        .task
        .as_ref()
        .expect("the confirmation carries its task");
    assert_eq!((task.column, task.bot_id.as_deref()), (Column::Open, None));
    assert_eq!(task.body, "Add magic link sign-in");

    let again = client
        .team_room_answer(&answer(&card.id, &offered.options[0].choice))
        .await
        .expect_err("a card takes one answer");
    assert_eq!(remote_code(&again), codes::TEAM_ROOM_CARD_ANSWERED);
    assert_eq!(tasks_of(&client, "alpha").await.len(), 1);
    let room = client
        .team_room_list(&TeamRoomListParams {
            team: "alpha".to_owned(),
            limit: None,
        })
        .await
        .expect("team.room_list");
    assert_eq!(room.messages.len(), 3);
    assert_eq!(room.messages[1], answered.card);
    stop(running).await;
}

#[tokio::test]
async fn a_card_sends_its_request_to_a_bot_or_to_another_team_todo() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    team_with(&client, "alpha", &[("Mara", false), ("Bruno", false)]).await;
    team_with(&client, "beta", &[("Iris", true), ("Nora", false)]).await;

    // Start off is kept on the card, so the bot's task waits in Todo and no agent starts.
    let first = client
        .team_room_post(&held("Parse the headers"))
        .await
        .expect("team.room_post");
    let card = &first.messages[1];
    let bruno = card.card.as_ref().expect("a card").options[1]
        .choice
        .clone();
    let to_bot = client
        .team_room_answer(&answer(&card.id, &bruno))
        .await
        .expect("team.room_answer");
    let [line] = to_bot.messages.as_slice() else {
        panic!("the bot's answer: {to_bot:?}");
    };
    assert_eq!(line.bot_name.as_deref(), Some("Bruno"));
    assert_eq!(
        line.text,
        "Added to the Board, not started: Parse the headers"
    );
    let task = line.task.as_ref().expect("its task");
    assert_eq!(
        (task.column, task.session_id.as_deref()),
        (Column::Open, None)
    );

    let second = client
        .team_room_post(&held("Review the copy"))
        .await
        .expect("team.room_post");
    let card = &second.messages[1];
    let beta = card
        .card
        .as_ref()
        .expect("a card")
        .options
        .iter()
        .find(|option| option.kind == RoomChoiceKind::Team)
        .expect("the other team")
        .choice
        .clone();
    let to_team = client
        .team_room_answer(&answer(&card.id, &beta))
        .await
        .expect("team.room_answer");
    assert_eq!(
        to_team.messages[0].text,
        "Sent to team beta, in its Todo: Review the copy"
    );
    let sent = tasks_of(&client, "beta").await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].title, "Review the copy");
    assert_eq!(sent[0].column, Column::Open);
    assert_eq!(sent[0].session_id, None, "it waits for that team");
    assert!(sent[0].bot_id.is_some(), "the other team's lead owns it");
    assert_eq!(tasks_of(&client, "alpha").await.len(), 1);
    stop(running).await;
}

#[tokio::test]
async fn a_lead_a_mention_or_an_only_bot_still_take_a_line_with_no_card() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    team_with_bots(&client).await;
    team_with(&client, "solo", &[("Iris", false)]).await;

    let to_lead = client
        .team_room_post(&held("Check the logs"))
        .await
        .expect("team.room_post");
    assert_eq!(to_lead.messages[1].bot_name.as_deref(), Some("Bruno"));
    assert!(to_lead.messages.iter().all(|line| line.card.is_none()));
    let mentioned = client
        .team_room_post(&held("@Mara check the docs"))
        .await
        .expect("team.room_post");
    assert_eq!(mentioned.messages[1].bot_name.as_deref(), Some("Mara"));
    let mut solo = held("Check the build");
    solo.team = "solo".to_owned();
    let only = client.team_room_post(&solo).await.expect("team.room_post");
    assert_eq!(only.messages[1].bot_name.as_deref(), Some("Iris"));
    assert!(only.messages[1].card.is_none());
    stop(running).await;
}

#[tokio::test]
async fn a_reply_to_a_persons_line_stays_a_task_and_keeps_the_quoted_line() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    team_with_bots(&client).await;

    let first = client
        .team_room_post(&held("The login page is slow"))
        .await
        .expect("team.room_post");
    let quoted = first.messages[0].id.clone();
    let mut reply = held("@Mara profile it");
    reply.reply_to = Some(quoted.clone());
    let posted = client.team_room_post(&reply).await.expect("team.room_post");
    let [person, answer] = posted.messages.as_slice() else {
        panic!("a line and its answer: {posted:?}");
    };
    assert_eq!(person.reply_to.as_deref(), Some(quoted.as_str()));
    let quote = person.quoted.as_ref().expect("the line shows its quote");
    assert_eq!(
        (quote.author, quote.text.as_str()),
        (RoomAuthor::Person, "The login page is slow")
    );
    assert_eq!(answer.bot_name.as_deref(), Some("Mara"));
    let task = answer.task.as_ref().expect("a reply to a person is a task");
    assert_eq!(task.title, "profile it");
    assert_eq!(
        task.body,
        "@Mara profile it

In reply to Person: The login page is slow"
    );

    let unknown = client
        .team_room_post(&{
            let mut params = held("And this?");
            params.reply_to = Some("no-line".to_owned());
            params
        })
        .await
        .expect_err("a reply to no line of the room");
    assert_eq!(remote_code(&unknown), codes::TEAM_ROOM_LINE_NOT_FOUND);
    let room = client
        .team_room_list(&TeamRoomListParams {
            team: "alpha".to_owned(),
            limit: None,
        })
        .await
        .expect("team.room_list");
    assert_eq!(room.messages.len(), 4, "the refused line is not stored");
    assert!(room.typing.is_empty(), "no bot works on a held task");
    stop(running).await;
}
