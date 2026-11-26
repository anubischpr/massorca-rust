//! A chat session's secret request cards (X-CH-07) against a scripted service: where an agent's request sits in its
//! transcript, what the card's buttons send, and how the card settles once the service answers.

use super::session_view_tests::{Rig, chat_s1, draw, kind, session_rig};
use crate::model::navigation::Page;
use crate::model::settings_sections::SettingsSection;
use crate::service::{Command, Refusal, Script, SecretCommand, SecretsUpdate, StoreUpdate};
use gpui::{Modifiers, TestAppContext, VisualTestContext};
use massorca_protocol::methods::chat::{
    ChatCursor, ChatEvent, ChatItem, ChatItemBody, ChatSnapshot, MessageState,
};
use massorca_protocol::methods::secrets::{SecretDecline, SecretRequest};
use tokio::sync::oneshot;

fn assistant(id: &str, position: u64, text: &str) -> ChatItem {
    ChatItem {
        id: id.to_owned(),
        position,
        seq: position,
        body: ChatItemBody::AssistantMessage {
            text: text.to_owned(),
            state: MessageState::Completed,
        },
        ..crate::service::fixtures::base_chat_item()
    }
}

fn snapshot(items: Vec<ChatItem>) -> StoreUpdate {
    let seq = items.last().map_or(0, |item| item.seq);
    StoreUpdate::Chat {
        session: "s1".to_owned(),
        event: ChatEvent::Snapshot(Box::new(ChatSnapshot {
            cursor: ChatCursor {
                epoch: "e1".to_owned(),
                seq,
            },
            items,
            ..crate::service::fixtures::base_chat_snapshot()
        })),
    }
}

/// A request for `OPENAI_API_KEY` in the rig's project `p1`, asked by `session` and in `state`.
fn request(id: &str, session: Option<&str>, state: &str) -> SecretRequest {
    SecretRequest {
        id: id.to_owned(),
        scope: "project:p1".to_owned(),
        name: "OPENAI_API_KEY".to_owned(),
        reason: "Calls the OpenAI API for the project.".to_owned(),
        session: session.map(str::to_owned),
        state: state.to_owned(),
        created_at_ms: 20,
        ..crate::service::fixtures::base_secret_request()
    }
}

fn requests(list: Vec<SecretRequest>) -> StoreUpdate {
    StoreUpdate::Secrets(SecretsUpdate::Requests(Ok(list)))
}

fn push(rig: &Rig, cx: &mut VisualTestContext, update: StoreUpdate) {
    rig.updates.send(update).expect("the store listens");
    cx.run_until_parked();
    draw(cx);
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let center = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("no element with selector {selector}"))
        .center();
    cx.simulate_click(center, Modifiers::default());
    cx.run_until_parked();
}

type DeclineCall = (
    SecretDecline,
    oneshot::Sender<Result<SecretRequest, Refusal>>,
);

/// Every decline the shell sent since the last look, with the reply that answers it.
fn decline_calls(script: &mut Script) -> Vec<DeclineCall> {
    let mut calls = Vec::new();
    while let Some(command) = script.next() {
        if let Command::Secret(SecretCommand::Decline { params, reply }) = command {
            calls.push((params, reply));
        }
    }
    calls
}

/// The request the open Add dialog answers, if a dialog is open.
fn dialog_request(rig: &Rig, cx: &mut VisualTestContext) -> Option<String> {
    rig.shell.read_with(cx, |shell, _| {
        shell
            .vault_dialog
            .as_ref()
            .and_then(|dialog| dialog.request.clone())
    })
}

/// A chat session of the rig with one message and the secret requests `list` held by the store.
fn chat_with(cx: &mut TestAppContext, list: Vec<SecretRequest>) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = session_rig(cx, vec![kind(false)], vec![chat_s1(false)]);
    push(
        &rig,
        cx,
        snapshot(vec![assistant("a1", 1, "I need a key.")]),
    );
    push(&rig, cx, requests(list));
    (rig, cx)
}

#[gpui::test]
fn a_pending_request_of_this_session_sits_in_its_transcript_with_add_and_decline(
    cx: &mut TestAppContext,
) {
    let (_rig, cx) = chat_with(cx, vec![request("r1", Some("s1"), "pending")]);

    assert!(cx.debug_bounds("transcript-row-secret-r1").is_some());
    assert!(cx.debug_bounds("button:secret-r1-add").is_some());
    assert!(cx.debug_bounds("button:secret-r1-decline").is_some());
}

#[gpui::test]
fn another_sessions_request_and_one_from_the_command_line_are_not_drawn_here(
    cx: &mut TestAppContext,
) {
    let (_rig, cx) = chat_with(
        cx,
        vec![
            request("r2", Some("s2"), "pending"),
            request("r3", None, "pending"),
        ],
    );

    assert!(cx.debug_bounds("transcript-row-secret-r2").is_none());
    assert!(cx.debug_bounds("transcript-row-secret-r3").is_none());
}

#[gpui::test]
fn add_opens_the_add_dialog_for_the_request_and_the_card_stays_while_it_is_open(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = chat_with(cx, vec![request("r1", Some("s1"), "pending")]);

    click(cx, "button:secret-r1-add");

    assert_eq!(dialog_request(&rig, cx).as_deref(), Some("r1"));
    assert!(cx.debug_bounds("transcript-row-secret-r1").is_some());
}

#[gpui::test]
fn decline_sends_secret_decline_for_the_request_and_the_card_settles_as_declined(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = chat_with(cx, vec![request("r1", Some("s1"), "pending")]);

    click(cx, "button:secret-r1-decline");
    let mut calls = decline_calls(&mut rig.script);
    assert_eq!(calls.len(), 1, "one decline is sent");
    let (params, reply) = calls.remove(0);
    assert_eq!(params.request, "r1");
    let _ = reply.send(Ok(request("r1", Some("s1"), "declined")));
    push(
        &rig,
        cx,
        requests(vec![request("r1", Some("s1"), "declined")]),
    );

    assert!(cx.debug_bounds("button:secret-r1-decline").is_none());
    assert!(cx.debug_bounds("transcript-row-secret-r1").is_some());
}

#[gpui::test]
fn a_stored_request_settles_into_a_row_whose_open_vault_link_opens_the_vault(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = chat_with(cx, vec![request("r1", Some("s1"), "stored")]);

    assert!(cx.debug_bounds("button:secret-r1-add").is_none());
    click(cx, "link:secret-r1-vault");

    let page = rig
        .shell
        .read_with(cx, |shell, _| shell.model().navigation().page);
    assert_eq!(page, Page::Settings);
    assert_eq!(
        rig.shell.read_with(cx, |shell, _| shell.settings.section),
        SettingsSection::Vault
    );
}

#[gpui::test]
fn a_team_request_offers_decline_but_no_add_here(cx: &mut TestAppContext) {
    let team = SecretRequest {
        scope: "team:t1".to_owned(),
        ..request("r1", Some("s1"), "pending")
    };
    let (rig, cx) = chat_with(cx, vec![team]);

    click(cx, "button:secret-r1-add");

    assert_eq!(
        dialog_request(&rig, cx),
        None,
        "a team's secret is not stored here"
    );
    assert!(cx.debug_bounds("button:secret-r1-decline").is_some());
}

#[gpui::test]
fn a_refused_answer_shows_its_reason_and_retry_opens_the_add_dialog_again(cx: &mut TestAppContext) {
    let (rig, cx) = chat_with(cx, vec![request("r1", Some("s1"), "pending")]);
    rig.shell.update(cx, |shell, cx| {
        shell.request_notice = Some(("r1".to_owned(), "The vault is locked.".to_owned()));
        cx.notify();
    });
    cx.run_until_parked();
    draw(cx);

    assert!(cx.debug_bounds("button:secret-r1-retry").is_some());
    click(cx, "button:secret-r1-retry");

    assert_eq!(dialog_request(&rig, cx).as_deref(), Some("r1"));
}
