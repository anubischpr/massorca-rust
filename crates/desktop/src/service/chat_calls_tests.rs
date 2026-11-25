//! The chat calls of the bridge (decision 0101): a start that falls back to a terminal, and what unwatching a session
//! leaves.

use std::collections::BTreeSet;

use massorca_protocol::codes;
use massorca_protocol::methods::agents::AgentStartParams;
use massorca_protocol::methods::chat::RunType;
use tokio::sync::mpsc::unbounded_channel;

use super::bridge::{Command, Script};
use super::chat::{ChatCommand, ChatMirrors, note_unwatch};
use super::fixtures::agent;
use super::{Refusal, ServiceHandle, StoreUpdate};

fn start_params() -> AgentStartParams {
    AgentStartParams {
        workspace: Some("w1".to_owned()),
        kind: "claude".to_owned(),
        prompt: Some("hi".to_owned()),
        args: Vec::new(),
        env: Default::default(),
        read_only: false,
        title: None,
        cols: Some(120),
        rows: Some(40),
        request_id: None,
        permission_mode: None,
        model: None,
        effort: None,
    }
}

/// The next call the handle sent, waiting for the task that sends it.
async fn next_call(script: &mut Script) -> Command {
    loop {
        if let Some(command) = script.next() {
            return command;
        }
        tokio::task::yield_now().await;
    }
}

fn refused(code: &str, message: &str) -> Refusal {
    Refusal::Failed {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

#[tokio::test]
async fn a_chat_start_sends_no_grid_and_opens_the_chat_session() {
    let (handle, mut script) = ServiceHandle::scripted();
    let task = tokio::spawn(async move { handle.start_session(start_params(), true).await });

    let Command::Chat(ChatCommand::Start { params, reply }) = next_call(&mut script).await else {
        panic!("a chat start is sent first");
    };
    assert_eq!((params.cols, params.rows), (None, None));
    assert_eq!(params.prompt.as_deref(), Some("hi"));
    let mut session = agent("s1", "w1", 1);
    session.run = RunType::Chat;
    let _ = reply.send(Ok(session));

    let started = task.await.expect("the task ends").expect("the start works");
    assert_eq!(started.session.run, RunType::Chat);
    assert_eq!(started.fell_back, None);
}

#[tokio::test]
async fn a_kind_that_cannot_chat_starts_a_terminal_session_and_says_why() {
    let (handle, mut script) = ServiceHandle::scripted();
    let task = tokio::spawn(async move { handle.start_session(start_params(), true).await });

    let Command::Chat(ChatCommand::Start { reply, .. }) = next_call(&mut script).await else {
        panic!("a chat start is sent first");
    };
    let _ = reply.send(Err(refused(
        codes::AGENT_CHAT_UNSUPPORTED,
        "claude 2.0.1 is outside the supported range",
    )));
    let Command::AgentStart { params, reply } = next_call(&mut script).await else {
        panic!("the terminal start follows");
    };
    assert_eq!(
        (params.cols, params.rows),
        (Some(120), Some(40)),
        "the terminal start keeps its grid"
    );
    let _ = reply.send(Ok(agent("s1", "w1", 1)));

    let started = task.await.expect("the task ends").expect("the start works");
    assert_eq!(started.session.run, RunType::Terminal);
    assert_eq!(
        started.fell_back.as_deref(),
        Some("Claude 2.0.1 is outside the supported range")
    );
}

#[tokio::test]
async fn any_other_refusal_of_a_chat_start_is_the_answer_and_starts_nothing_else() {
    let (handle, mut script) = ServiceHandle::scripted();
    let task = tokio::spawn(async move { handle.start_session(start_params(), true).await });

    let Command::Chat(ChatCommand::Start { reply, .. }) = next_call(&mut script).await else {
        panic!("a chat start is sent first");
    };
    let _ = reply.send(Err(refused("agent.workspace_busy", "Busy")));

    let answer = task.await.expect("the task ends");
    assert_eq!(
        answer.expect_err("refused").code(),
        Some("agent.workspace_busy")
    );
    assert!(script.next().is_none(), "no terminal start was tried");
}

#[tokio::test]
async fn a_terminal_start_never_asks_for_a_chat() {
    let (handle, mut script) = ServiceHandle::scripted();
    let task = tokio::spawn(async move { handle.start_session(start_params(), false).await });

    let Command::AgentStart { reply, .. } = next_call(&mut script).await else {
        panic!("a terminal start is sent");
    };
    let _ = reply.send(Ok(agent("s1", "w1", 1)));

    assert!(task.await.expect("the task ends").is_ok());
}

#[test]
fn unwatching_forgets_the_session_and_tells_the_store_to_drop_its_transcript() {
    let (updates, mut seen) = unbounded_channel();
    let mirrors = ChatMirrors::default();
    let mut watching = BTreeSet::from(["s1".to_owned(), "s2".to_owned()]);

    note_unwatch("s1", &mut watching, &mirrors, &updates);

    assert_eq!(watching, BTreeSet::from(["s2".to_owned()]));
    assert_eq!(
        seen.try_recv().ok(),
        Some(StoreUpdate::ChatClosed("s1".to_owned()))
    );
}
