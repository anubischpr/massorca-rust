//! The Conference room over the wire: a person's line that names a day and a time becomes a one-time routine of its
//! bot, with the bot's answer about it (decision 0121), and a line with start off waits in Todo. None of these lines
//! starts an agent; a refused routine, which becomes a task, is covered by the CLI tests with the fake agent.

use massorca_protocol::methods::events::TOPIC_TEAM_CHANGED;
use massorca_protocol::methods::tasks::Column;
use massorca_protocol::methods::team_room::{RoomAuthor, TeamRoomListParams, TeamRoomPostParams};
use massorca_protocol::methods::teams::{BotAddParams, RoutineCadence, TeamCreateParams, TeamRef};

use super::common::{connect, start, stop, temp_home};

fn post(text: &str, bot: Option<&str>) -> TeamRoomPostParams {
    TeamRoomPostParams {
        team: "alpha".to_owned(),
        text: text.to_owned(),
        bot: bot.map(str::to_owned),
        to: None,
        session: None,
        agent_token: None,
        start: None,
    }
}

async fn team_with_bots(client: &massorca_client::Client) {
    client
        .team_create(&TeamCreateParams {
            name: "alpha".to_owned(),
            purpose: None,
            character: None,
            access: None,
            workspaces: None,
        })
        .await
        .expect("team.create");
    for (name, lead) in [("Mara", false), ("Bruno", true)] {
        client
            .team_bot_add(&BotAddParams {
                team: "alpha".to_owned(),
                name: name.to_owned(),
                job: "Keeps the docs".to_owned(),
                kind: "claude".to_owned(),
                model: None,
                effort: None,
                lead,
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
