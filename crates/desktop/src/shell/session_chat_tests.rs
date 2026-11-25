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
    Approval, ApprovalDecision, ApprovalScope, ApprovalState, ApprovalSuggestion,
    ChatApprovalAnswerParams, ChatBatch, ChatCursor, ChatEvent, ChatHistoryPage, ChatHistoryParams,
    ChatInterrupted, ChatItem, ChatItemBody, ChatSnapshot, ChatUpdate, MessageState,
};
use tokio::sync::oneshot;
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

/// A pending permission request in the transcript, as the stream delivers it: item `ap` at `seq` 5.
fn pending_approval() -> ChatItem {
    ChatItem {
        id: "ap".to_owned(),
        position: 3,
        seq: 5,
        turn: None,
        parent: None,
        at_ms: 0,
        body: ChatItemBody::Approval(Approval {
            request_id: "r-1".to_owned(),
            tool: "Bash".to_owned(),
            input_summary: "touch probe.txt".to_owned(),
            truncated: false,
            tool_use_id: Some("toolu_1".to_owned()),
            can_always: true,
            suggestions: vec![ApprovalSuggestion {
                kind: "addRules".to_owned(),
                rules: vec!["Bash(touch probe.txt)".to_owned()],
                destination: "localSettings".to_owned(),
            }],
            state: ApprovalState::Pending,
            scope: None,
            settled_at_ms: None,
        }),
    }
}

fn answer_calls(
    script: &mut Script,
) -> Vec<(
    ChatApprovalAnswerParams,
    oneshot::Sender<Result<ChatItem, Refusal>>,
)> {
    let mut answers = Vec::new();
    while let Some(command) = script.next() {
        if let Command::Chat(ChatCommand::Answer { params, reply }) = command {
            answers.push((params, reply));
        }
    }
    answers
}

#[gpui::test]
fn the_card_shows_what_always_writes_and_each_button_answers_with_the_seq_it_showed(
    cx: &mut TestAppContext,
) {
    for (button, decision, scope) in [
        ("Approve", ApprovalDecision::Allow, Some(ApprovalScope::Once)),
        ("Always", ApprovalDecision::Allow, Some(ApprovalScope::Always)),
        ("Deny", ApprovalDecision::Deny, None),
    ] {
        let (mut rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);
        let items = vec![pending_approval()];
        push(&rig, cx, snapshot("s1", items, false));
        assert!(
            cx.debug_bounds("approval-always-ap").is_some(),
            "the rules an Always answer writes are listed"
        );
        chat_calls(&mut rig.script);

        let selector: &'static str = match button {
            "Approve" => "button:ap-Approve",
            "Always" => "button:ap-Always",
            _ => "button:ap-Deny",
        };
        let center = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is not drawn"))
            .center();
        cx.simulate_click(center, gpui::Modifiers::default());
        cx.run_until_parked();

        let mut answers = answer_calls(&mut rig.script);
        assert_eq!(answers.len(), 1, "{button}: one answer goes out");
        let (params, _reply) = answers.remove(0);
        assert_eq!(params.session, "s1");
        assert_eq!(params.request_id, "r-1");
        assert_eq!(params.decision, decision, "{button}");
        assert_eq!(params.scope, scope, "{button}");
        assert_eq!(params.seq, Some(5), "{button}: the seq of the item shown");
    }
}

#[gpui::test]
fn a_second_click_while_the_answer_is_on_its_way_sends_nothing(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);
    push(&rig, cx, snapshot("s1", vec![pending_approval()], false));
    chat_calls(&mut rig.script);
    let center = cx
        .debug_bounds("button:ap-Approve")
        .expect("Approve is drawn")
        .center();

    cx.simulate_click(center, gpui::Modifiers::default());
    cx.simulate_click(center, gpui::Modifiers::default());
    cx.run_until_parked();

    assert_eq!(answer_calls(&mut rig.script).len(), 1);
}

#[gpui::test]
fn an_answer_to_a_request_that_changed_or_was_settled_says_so_in_a_short_note(
    cx: &mut TestAppContext,
) {
    for (code, note) in [
        (
            "agent.approval_changed",
            "That request changed before your answer arrived. Read it again and answer.",
        ),
        (
            "agent.approval_settled",
            "That request was already answered.",
        ),
        (
            "agent.busy",
            "Could not send your answer: The agent is busy",
        ),
    ] {
        let (mut rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);
        push(&rig, cx, snapshot("s1", vec![pending_approval()], false));
        chat_calls(&mut rig.script);
        let center = cx
            .debug_bounds("button:ap-Approve")
            .expect("Approve is drawn")
            .center();
        cx.simulate_click(center, gpui::Modifiers::default());
        cx.run_until_parked();

        let (_, reply) = answer_calls(&mut rig.script).remove(0);
        reply
            .send(Err(Refusal::Failed {
                code: code.to_owned(),
                message: "the agent is busy".to_owned(),
            }))
            .expect("the shell waits for the answer");
        cx.run_until_parked();

        let shown = rig
            .shell
            .read_with(cx, |shell, _| shell.session_notices.get("s1").cloned());
        assert_eq!(shown.as_deref(), Some(note), "{code}");
    }
}

#[gpui::test]
fn the_item_the_stream_settles_replaces_the_card_with_its_one_line(cx: &mut TestAppContext) {
    let (rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);
    push(&rig, cx, snapshot("s1", vec![pending_approval()], false));
    assert!(cx.debug_bounds("button:ap-Approve").is_some());

    let mut settled = pending_approval();
    settled.seq = 6;
    if let ChatItemBody::Approval(approval) = &mut settled.body {
        approval.state = ApprovalState::Allowed;
        approval.scope = Some(ApprovalScope::Always);
    }
    push(
        &rig,
        cx,
        StoreUpdate::Chat {
            session: "s1".to_owned(),
            event: ChatEvent::Batch(ChatBatch {
                cursor: cursor(6),
                updates: vec![ChatUpdate::Item { item: settled }],
                activity: None,
            }),
        },
    );

    assert!(cx.debug_bounds("button:ap-Approve").is_none());
    assert!(cx.debug_bounds("transcript-row-ap").is_some());
}

#[gpui::test]
fn resume_in_an_ended_chat_starts_it_with_no_grid_and_follows_the_transcript_again(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = session_rig(cx, vec![kind(true)], vec![chat_s1(true)]);
    chat_calls(&mut rig.script);

    emit(&rig, cx, "", |_| ComposerEvent::Resume);

    let mut resumed = None;
    while let Some(command) = rig.script.next() {
        if let Command::AgentResume { params, reply } = command {
            resumed = Some((params, reply));
        }
    }
    let (params, reply) = resumed.expect("Resume asks the service for a new run");
    assert_eq!(params.session, "s1");
    assert_eq!(
        (params.cols, params.rows),
        (None, None),
        "a chat run has no terminal to size"
    );
    let live = chat_s1(false);
    reply
        .send(Ok(live.clone()))
        .expect("the shell waits for the answer");
    rig.updates
        .send(StoreUpdate::AgentChanged(live))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);

    let calls = chat_calls(&mut rig.script);
    assert!(
        calls
            .iter()
            .any(|call| matches!(call, ChatCommand::Watch { session } if session == "s1")),
        "the stream is asked for again: {calls:?}"
    );
}

#[gpui::test]
fn resume_in_an_ended_terminal_session_still_sizes_its_terminal(cx: &mut TestAppContext) {
    let (mut rig, cx) = session_rig(cx, vec![kind(true)], vec![s1(true, None)]);

    emit(&rig, cx, "", |_| ComposerEvent::Resume);

    let mut grid = None;
    while let Some(command) = rig.script.next() {
        if let Command::AgentResume { params, .. } = command {
            grid = Some((params.cols, params.rows));
        }
    }
    let (cols, rows) = grid.expect("Resume asks the service for a new run");
    assert!(cols.is_some() && rows.is_some());
}
