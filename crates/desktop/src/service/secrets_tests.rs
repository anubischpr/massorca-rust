use massorca_protocol::methods::secrets::{SecretSet, SecretStatus};
use tokio::sync::mpsc;

use super::secrets::{SecretCommand, SecretsFreshness};
use super::update::{SecretsUpdate, StoreUpdate};

#[test]
fn a_command_that_stores_a_value_never_prints_it() {
    let (reply, _answer) = tokio::sync::oneshot::channel();
    let command = SecretCommand::Set {
        params: SecretSet {
            scope: "global".to_owned(),
            name: "TOKEN".to_owned(),
            value: "hidden-test-value".to_owned(),
            replace: false,
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
        name_clashes: Vec::new(),
    }))
}

fn list() -> SecretsUpdate {
    SecretsUpdate::List(Ok(Vec::new()))
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
fn the_two_answers_of_one_read_are_sent_in_order() {
    let order = SecretsFreshness::default();
    let (updates, mut received) = mpsc::unbounded_channel();
    let number = order.request();
    order.deliver(number, &updates, [status(2), list()]);
    assert_eq!(
        drain(&mut received),
        vec![
            StoreUpdate::Secrets(status(2)),
            StoreUpdate::Secrets(list())
        ]
    );
}

#[test]
fn an_older_read_that_answers_late_never_replaces_a_newer_one() {
    let order = SecretsFreshness::default();
    let (updates, mut received) = mpsc::unbounded_channel();
    let older = order.request();
    let newer = order.request();
    order.deliver(newer, &updates, [status(3), list()]);
    order.deliver(older, &updates, [status(1), list()]);
    assert_eq!(
        drain(&mut received),
        vec![
            StoreUpdate::Secrets(status(3)),
            StoreUpdate::Secrets(list())
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
    order.deliver(first, &updates, [status(1), list()]);
    order.deliver(second, &updates, [status(2), list()]);
    assert_eq!(drain(&mut received).len(), 4);
}
