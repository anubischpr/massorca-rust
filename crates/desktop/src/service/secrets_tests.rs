use massorca_protocol::methods::secrets::{SecretDecline, SecretLink, SecretSet, SecretStatus};
use tokio::sync::{mpsc, oneshot};

use super::bridge::{Command, ServiceHandle};
use super::secrets::{SecretCommand, SecretsFreshness};
use super::update::{SecretsUpdate, StoreUpdate};

#[test]
fn a_command_that_stores_a_value_never_prints_it() {
    let (reply, _answer) = oneshot::channel();
    let command = SecretCommand::Set {
        params: SecretSet {
            ..crate::service::fixtures::base_secret_set()
        },
        reply,
    };
    let printed = format!("{command:?}");
    assert!(!printed.contains("hidden-test-value"), "{printed}");
    assert!(printed.contains("[redacted]"), "{printed}");
}

fn status(count: u64) -> SecretsUpdate {
    SecretsUpdate::Status(Ok(SecretStatus {
        protection: "file".to_owned(),
        state: "ready".to_owned(),
        count,
        ..crate::service::fixtures::base_secret_status()
    }))
}

fn list() -> SecretsUpdate {
    SecretsUpdate::List(Ok(Vec::new()))
}

fn requests() -> SecretsUpdate {
    SecretsUpdate::Requests(Ok(Vec::new()))
}

/// Everything sent so far, in the order it was sent.
fn drain(received: &mut mpsc::UnboundedReceiver<StoreUpdate>) -> Vec<StoreUpdate> {
    let mut sent = Vec::new();
    while let Ok(update) = received.try_recv() {
        sent.push(update);
    }
    sent
}

#[test]
fn the_three_answers_of_one_read_are_sent_in_order() {
    let order = SecretsFreshness::default();
    let (updates, mut received) = mpsc::unbounded_channel();
    let number = order.request();
    order.deliver(number, &updates, [status(2), list(), requests()]);
    assert_eq!(
        drain(&mut received),
        vec![
            StoreUpdate::Secrets(status(2)),
            StoreUpdate::Secrets(list()),
            StoreUpdate::Secrets(requests())
        ]
    );
}

#[test]
fn an_older_read_that_answers_late_never_replaces_a_newer_one() {
    let order = SecretsFreshness::default();
    let (updates, mut received) = mpsc::unbounded_channel();
    let older = order.request();
    let newer = order.request();
    order.deliver(newer, &updates, [status(3), list(), requests()]);
    order.deliver(older, &updates, [status(1), list(), requests()]);
    assert_eq!(
        drain(&mut received),
        vec![
            StoreUpdate::Secrets(status(3)),
            StoreUpdate::Secrets(list()),
            StoreUpdate::Secrets(requests())
        ],
        "the stale read sends nothing"
    );
}

#[test]
fn a_newer_read_still_lands_after_an_older_one_delivered() {
    let order = SecretsFreshness::default();
    let (updates, mut received) = mpsc::unbounded_channel();
    let first = order.request();
    let second = order.request();
    order.deliver(first, &updates, [status(1), list(), requests()]);
    order.deliver(second, &updates, [status(2), list(), requests()]);
    assert_eq!(drain(&mut received).len(), 6);
}

/// The service's side of a scripted handle: a call sent by the UI reaches it as one command, once the task has run.
#[tokio::test]
async fn an_answer_that_stores_a_value_sends_the_value_to_the_service_only() {
    let (handle, mut script) = ServiceHandle::scripted();
    let params = SecretSet {
        name: "OPENAI_API_KEY".to_owned(),
        value: "sk-scripted-value".to_owned(),
        ..crate::service::fixtures::base_secret_set()
    };
    let call = tokio::spawn(async move { handle.secret_set(params).await });
    tokio::task::yield_now().await;
    let Some(Command::Secret(SecretCommand::Set { params, .. })) = script.next() else {
        panic!("the service received no set");
    };
    assert_eq!(params.name, "OPENAI_API_KEY");
    assert_eq!(params.value, "sk-scripted-value");
    call.abort();
}

#[tokio::test]
async fn an_answer_that_links_names_the_scope_that_owns_the_secret() {
    let (handle, mut script) = ServiceHandle::scripted();
    let params = SecretLink {
        ..crate::service::fixtures::base_secret_link()
    };
    let call = tokio::spawn(async move { handle.secret_link(params).await });
    tokio::task::yield_now().await;
    let Some(Command::Secret(SecretCommand::Link { params, .. })) = script.next() else {
        panic!("the service received no link");
    };
    assert_eq!(params.scope, "project:p1");
    assert_eq!(params.from_scope, "global");
    call.abort();
}

#[tokio::test]
async fn a_decline_names_the_request_and_carries_no_value() {
    let (handle, mut script) = ServiceHandle::scripted();
    let call = tokio::spawn(async move {
        handle
            .secret_decline(SecretDecline {
                ..crate::service::fixtures::base_secret_decline()
            })
            .await
    });
    tokio::task::yield_now().await;
    let Some(Command::Secret(SecretCommand::Decline { params, .. })) = script.next() else {
        panic!("the service received no decline");
    };
    assert_eq!(params.request, "request-1");
    call.abort();
}
