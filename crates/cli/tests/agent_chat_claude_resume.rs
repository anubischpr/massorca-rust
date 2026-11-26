//! Claude chat sessions end to end (decision 0101), resume: a stopped chat resumes as a chat with its session id, and
//! its journal carries on in its epoch.

mod chat_claude_support;
mod harness;

use chat_claude_support::*;
use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{self, ChatItemBody, TurnState};
use serde_json::json;

#[test]
fn a_stopped_claude_chat_resumes_as_a_chat_with_its_session_id_and_its_journal_carries_on() {
    let (sandbox, logs) = sandbox(
        &[await_input("user"), result(true)],
        &[
            await_input("user"),
            assistant("m9", json!([{ "type": "text", "text": "Resumed." }])),
            result(true),
        ],
        &[],
    );
    let session = start_chat(&sandbox, "first");
    let first_epoch = runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        transcript
            .until("the first turn", |items| {
                (turns(items) == [(TurnState::Completed, Some(Outcome::Success))]).then_some(())
            })
            .await;
        let page = client
            .call::<_, massorca_protocol::methods::chat::ChatHistoryPage>(
                chat::HISTORY,
                &massorca_protocol::methods::chat::ChatHistoryParams {
                    session: session.clone(),
                    before: None,
                    limit: None,
                },
            )
            .await
            .expect("agent.chat.history");
        page.epoch
    });
    let stopped = sandbox.json_ok("default", &["agent", "stop", &session]);
    assert_eq!(stopped["execution"]["state"], "exited", "{stopped}");

    let resumed = sandbox.json_ok("default", &["agent", "resume", &session]);
    assert_eq!(resumed["run"], "chat", "{resumed}");
    assert_eq!(resumed["execution"]["state"], "live", "{resumed}");
    let sent = sandbox.json_ok("default", &["agent", "prompt", &session, "second"]);
    assert_eq!(sent["receipt"], "sent_to_chat", "{sent}");
    runtime().block_on(async {
        let client = connect(sandbox.home()).await;
        let mut transcript = Transcript::open(&client, &session).await;
        let items = transcript
            .until("the resumed turn", |items| {
                (turns(items).len() == 2 && turns(items)[1].0 == TurnState::Completed)
                    .then(|| items.to_vec())
            })
            .await;
        assert!(items.iter().any(|item| matches!(&item.body,
            ChatItemBody::AssistantMessage { text, .. } if text == "Resumed.")));
        let page = client
            .call::<_, massorca_protocol::methods::chat::ChatHistoryPage>(
                chat::HISTORY,
                &massorca_protocol::methods::chat::ChatHistoryParams {
                    session: session.clone(),
                    before: None,
                    limit: None,
                },
            )
            .await
            .expect("agent.chat.history");
        assert_eq!(page.epoch, first_epoch, "the journal keeps its epoch");
    });
    let runs = logs.runs();
    assert_eq!(runs.len(), 2, "{runs:?}");
    let first_ref = runs[0]
        .iter()
        .skip_while(|arg| *arg != "--session-id")
        .nth(1)
        .expect("the session id");
    let resume_at = runs[1]
        .iter()
        .position(|arg| arg == "--resume")
        .expect("a resume argument");
    assert_eq!(&runs[1][resume_at + 1], first_ref);
    assert_eq!(runs[1][0], "-p");
}
