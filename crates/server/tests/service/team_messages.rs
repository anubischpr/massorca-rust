//! `team.message_send` over the wire (decision 0129). Only a bot's live agent run sends: a call without a run is refused
//! with its own code, and a token no live run holds with `permission.denied`. The edge and settlement rules are unit
//! tests of the service's module.

use massorca_protocol::methods::team_room::{
    self, MessagePriority, MessageType, TeamMessageSendParams, TeamMessageSent, WorkerOutcome,
};
use massorca_protocol::{capabilities, codes};

use super::common::{connect, remote_code, start, stop, temp_home};

fn send(session: Option<&str>, token: Option<&str>) -> TeamMessageSendParams {
    TeamMessageSendParams {
        to: "parent".to_owned(),
        kind: MessageType::WorkerDone,
        priority: MessagePriority::Normal,
        subject: "Lexer".to_owned(),
        body: "Done".to_owned(),
        outcome: Some(WorkerOutcome::Succeeded),
        task: None,
        thread: None,
        reply_to: None,
        session: session.map(str::to_owned),
        agent_token: token.map(str::to_owned),
    }
}

#[tokio::test]
async fn a_message_outside_a_bot_run_is_refused_with_its_own_code() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let sent = client
        .call::<_, TeamMessageSent>(team_room::MESSAGE_SEND, &send(None, None))
        .await
        .expect_err("no run");
    assert_eq!(remote_code(&sent), codes::TEAM_MESSAGE_NOT_A_BOT);
    stop(running).await;
}

#[tokio::test]
async fn a_run_token_no_live_run_holds_cannot_send() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let sent = client
        .call::<_, TeamMessageSent>(
            team_room::MESSAGE_SEND,
            &send(Some("no-such-session"), Some("token")),
        )
        .await
        .expect_err("no live run");
    assert_eq!(remote_code(&sent), codes::PERMISSION_DENIED);
    stop(running).await;
}

#[test]
fn the_messages_capability_is_named_in_the_protocol() {
    assert_eq!(capabilities::TEAM_MESSAGES, "team-messages/1");
}
