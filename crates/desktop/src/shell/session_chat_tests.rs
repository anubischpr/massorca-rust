//! A chat session's view against a scripted service (decision 0101): what follows its transcript, what pages its
//! history, and what its composer and approvals send. Each test answers the calls by hand.

use super::session_view_tests::{Rig, chat_s1, draw, kind, queue_rig, s1, session_rig};
use super::*;
use crate::service::{ChatCommand, Command, QueueCommand, Refusal, Script, StoreUpdate};
use gpui::{Entity, TestAppContext, VisualTestContext};
use massorca_protocol::methods::agents::{
    Activity, ActivitySource, ActivityState, AgentQueueList, AgentSession, PromptReceipt,
    ReceiptKind,
};
use massorca_protocol::methods::chat::{
    ChatCursor, ChatEvent, ChatHistoryPage, ChatHistoryParams, ChatInterrupted, ChatItem,
    ChatItemBody, ChatSnapshot, MessageState,
};
use massorca_ui::components::composer::{Composer, ComposerEvent};

fn cursor(seq: u64) -> ChatCursor {
    ChatCursor {
        epoch: "e1".to_owned(),
        seq,
    }
}

fn message(id: &str, position: u64, text: &str) -> ChatItem {
    ChatItem {
        id: id.to_owned(),
        position,
        seq: position,
        turn: None,
        parent: None,
        at_ms: 0,
        body: ChatItemBody::AssistantMessage {
            text: text.to_owned(),
            state: MessageState::Completed,
        },
    }
}

fn snapshot(session: &str, items: Vec<ChatItem>, has_older: bool) -> StoreUpdate {
    let head = items.last().map_or(0, |item| item.seq);
    StoreUpdate::Chat {
        session: session.to_owned(),
        event: ChatEvent::Snapshot(Box::new(ChatSnapshot {
            cursor: cursor(head),
            items,
            has_older,
            latest_turn: None,
            activity: None,
        })),
    }
}

/// Every chat call the shell sent since the last look, in order.
fn chat_calls(script: &mut Script) -> Vec<ChatCommand> {
    let mut calls = Vec::new();
    while let Some(command) = script.next() {
        if let Command::Chat(call) = command {
            calls.push(call);
        }
    }
    calls
}

fn push(rig: &Rig, cx: &mut VisualTestContext, update: StoreUpdate) {
    rig.updates.send(update).expect("the store listens");
    cx.run_until_parked();
    draw(cx);
}

#[gpui::test]
fn showing_a_chat_session_follows_its_transcript_and_closing_the_view_stops(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);

    let calls = chat_calls(&mut rig.script);
    assert!(
        matches!(calls.as_slice(), [ChatCommand::Watch { session }] if session == "s1"),
        "the open chat is followed once: {calls:?}"
    );
    draw(cx);
    assert!(chat_calls(&mut rig.script).is_empty(), "a redraw asks nothing");

    rig.shell.update(cx, |shell, cx| {
        shell.model.close_session();
        cx.notify();
    });
    cx.run_until_parked();
    draw(cx);

    let calls = chat_calls(&mut rig.script);
    assert!(
        matches!(calls.as_slice(), [ChatCommand::Unwatch { session }] if session == "s1"),
        "closing the view stops the stream: {calls:?}"
    );
}

#[gpui::test]
fn a_terminal_session_follows_no_transcript(cx: &mut TestAppContext) {
    let (mut rig, _cx) = session_rig(cx, vec![kind(false)], vec![s1(false, None)]);

    assert!(chat_calls(&mut rig.script).is_empty());
}

#[gpui::test]
fn the_streams_snapshot_and_batches_draw_in_the_chat_view(cx: &mut TestAppContext) {
    let (rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);

    push(
        &rig,
        cx,
        snapshot("s1", vec![message("a", 1, "Reading it now.")], false),
    );

    assert!(cx.debug_bounds("transcript-row-a").is_some());
}

fn history_calls(script: &mut Script) -> Vec<ChatHistoryParams> {
    chat_calls(script)
        .into_iter()
        .filter_map(|call| match call {
            ChatCommand::History { params, .. } => Some(params),
            _ => None,
        })
        .collect()
}

#[gpui::test]
fn older_items_are_asked_for_once_from_the_oldest_held_position_and_join_the_view(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);
    push(
        &rig,
        cx,
        snapshot("s1", vec![message("c", 8, "three"), message("d", 9, "four")], true),
    );
    chat_calls(&mut rig.script);

    rig.shell.update(cx, |shell, cx| shell.load_older_chat("s1", cx));
    rig.shell.update(cx, |shell, cx| shell.load_older_chat("s1", cx));
    cx.run_until_parked();

    let mut asked = Vec::new();
    let mut reply = None;
    while let Some(command) = rig.script.next() {
        if let Command::Chat(ChatCommand::History { params, reply: sender }) = command {
            asked.push(params);
            reply = Some(sender);
        }
    }
    assert_eq!(asked.len(), 1, "a second ask waits for the first: {asked:?}");
    assert_eq!(asked[0].before, Some(cursor(8)), "before the oldest held item");
    let page = ChatHistoryPage {
        epoch: "e1".to_owned(),
        items: vec![message("a", 2, "one")],
        has_older: false,
    };
    // The bridge adds the page to the store, then answers the caller.
    rig.updates
        .send(StoreUpdate::ChatHistory {
            session: "s1".to_owned(),
            page: page.clone(),
        })
        .expect("the store listens");
    reply
        .expect("the history call is waiting")
        .send(Ok(page))
        .expect("the shell waits for the page");
    cx.run_until_parked();
    draw(cx);

    assert!(cx.debug_bounds("transcript-row-a").is_some());
    rig.shell.update(cx, |shell, cx| shell.load_older_chat("s1", cx));
    assert!(
        history_calls(&mut rig.script).is_empty(),
        "nothing older is left to ask for"
    );
}

/// The chat session `s1` while its agent works.
fn working_chat() -> AgentSession {
    let mut session = chat_s1(false);
    session.activity = Some(Activity {
        state: ActivityState::Working,
        outcome: None,
        source: ActivitySource::Hook,
        at_ms: 1,
        seen_at_ms: None,
    });
    session
}

fn composer_of(rig: &Rig, cx: &mut VisualTestContext) -> Entity<Composer> {
    rig.shell
        .update(cx, |shell, cx| shell.session_composer("s1", cx))
}

fn draft_of(composer: &Entity<Composer>, cx: &VisualTestContext) -> String {
    composer.read_with(cx, |composer, cx| composer.draft(cx))
}

/// Puts `text` in the chat session's composer and makes it emit `event`, as the composer does for the key or the button.
fn emit(
    rig: &Rig,
    cx: &mut VisualTestContext,
    text: &str,
    event: impl FnOnce(String) -> ComposerEvent,
) {
    let composer = composer_of(rig, cx);
    composer.update(cx, |composer, cx| {
        composer.set_draft(text, cx);
        cx.emit(event(text.to_owned()));
    });
    cx.run_until_parked();
}

#[gpui::test]
fn send_in_a_chat_session_is_an_agent_prompt_the_service_receipts_as_sent_to_chat(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);

    emit(&rig, cx, "now the tests", ComposerEvent::Send);

    let mut sent = None;
    while let Some(command) = rig.script.next() {
        if let Command::AgentPrompt { params, reply } = command {
            sent = Some((params, reply));
        }
    }
    let (params, reply) = sent.expect("the text goes out as agent.prompt");
    assert_eq!(
        (params.session.as_str(), params.text.as_str(), params.force),
        ("s1", "now the tests", false)
    );
    reply
        .send(Ok(PromptReceipt {
            receipt: ReceiptKind::SentToChat,
            bytes: 13,
        }))
        .expect("the shell waits for the receipt");
    cx.run_until_parked();
    assert_eq!(draft_of(&composer_of(&rig, cx), cx), "", "accepted: cleared");
}

#[gpui::test]
fn while_a_turn_runs_send_queues_the_prompt_as_it_does_in_a_terminal_session(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = queue_rig(
        cx,
        vec![kind(false)],
        vec![working_chat()],
        Some(vec![AgentQueueList {
            session: "s1".to_owned(),
            items: Vec::new(),
        }]),
    );

    emit(&rig, cx, "and the docs", ComposerEvent::Queue);

    let mut queued = None;
    while let Some(command) = rig.script.next() {
        if let Command::AgentQueue(QueueCommand::Add { params, .. }) = command {
            queued = Some(params);
        }
    }
    let params = queued.expect("the text is queued through agent.queue");
    assert_eq!(
        (params.session.as_str(), params.text.as_str()),
        ("s1", "and the docs")
    );
}

#[gpui::test]
fn stop_in_a_working_chat_interrupts_the_turn_and_never_stops_the_run(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(cx, vec![kind(false)], vec![working_chat()]);
    chat_calls(&mut rig.script);

    emit(&rig, cx, "", |_| ComposerEvent::Stop);

    let mut interrupted = None;
    while let Some(command) = rig.script.next() {
        match command {
            Command::Chat(ChatCommand::Interrupt { session, reply }) => {
                interrupted = Some((session, reply));
            }
            Command::AgentStop { .. } => panic!("the run must stay"),
            _ => {}
        }
    }
    let (session, reply) = interrupted.expect("the turn is interrupted");
    assert_eq!(session, "s1");
    reply
        .send(Ok(ChatInterrupted {
            interrupted: true,
            turn: None,
        }))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    let notice = rig
        .shell
        .read_with(cx, |shell, _| shell.session_notices.get("s1").cloned());
    assert_eq!(notice, None, "a worked interrupt leaves no note");
}

#[gpui::test]
fn a_refused_interrupt_says_why_above_the_composer(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(cx, vec![kind(false)], vec![working_chat()]);

    emit(&rig, cx, "", |_| ComposerEvent::Stop);

    let mut reply = None;
    while let Some(command) = rig.script.next() {
        if let Command::Chat(ChatCommand::Interrupt { reply: sender, .. }) = command {
            reply = Some(sender);
        }
    }
    reply
        .expect("the turn is interrupted")
        .send(Err(Refusal::Failed {
            code: "agent.not_chat".to_owned(),
            message: "not a chat".to_owned(),
        }))
        .expect("the shell waits for the answer");
    cx.run_until_parked();
    let notice = rig
        .shell
        .read_with(cx, |shell, _| shell.session_notices.get("s1").cloned());
    assert_eq!(notice.as_deref(), Some("Could not stop the turn: Not a chat"));
}

#[gpui::test]
fn stop_in_an_idle_live_chat_still_ends_the_run(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);
    chat_calls(&mut rig.script);

    emit(&rig, cx, "", |_| ComposerEvent::Stop);

    let mut stopped = false;
    while let Some(command) = rig.script.next() {
        stopped |= matches!(command, Command::AgentStop { .. });
    }
    assert!(stopped, "with no turn to end, Stop ends the run");
}

#[gpui::test]
fn the_chat_composer_hides_attach_and_the_model_chip_until_the_service_offers_them(
    cx: &mut TestAppContext,
) {
    let (_rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);

    assert!(cx.debug_bounds("composer-primary").is_some());
    assert!(cx.debug_bounds("composer-attach").is_none());
    assert!(cx.debug_bounds("model-picker-trigger").is_none());
}
