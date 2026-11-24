//! Events: subscriptions receive what the bus publishes, cancelling stops them, and shutdown ends them with a reason.

use std::time::Duration;

use massorca_protocol::methods::events::{self, ChangeEvent};
use massorca_protocol::methods::service;
use massorca_protocol::{Message, codes};
use serde_json::Value;
use tokio::time::timeout;

use super::common::{Raw, connect, hello, request, start, stop, temp_home};

#[tokio::test]
async fn subscribers_receive_published_events() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let mut subscription = client
        .subscribe_events()
        .await
        .expect("events.subscribe answers");

    let event = ChangeEvent {
        topic: events::TOPIC_PROJECT_ADDED.to_owned(),
        id: "project-1".to_owned(),
    };
    running.events().publish(event.clone());

    let received = timeout(
        super::common::REPLY_WAIT,
        subscription.next_as::<ChangeEvent>(),
    )
    .await
    .expect("the event arrives")
    .expect("the stream is open")
    .expect("the event decodes");
    assert_eq!(received, event);
    stop(running).await;
}

#[tokio::test]
async fn a_cancelled_stream_sends_nothing_more() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let mut raw = Raw::open(home.path(), "default").await;
    raw.send(&hello()).await;
    assert!(
        matches!(raw.recv().await, Some(Message::Welcome { .. })),
        "the welcome comes first"
    );

    raw.send(&request(1, events::SUBSCRIBE, Value::Null)).await;
    let stream = subscribed_stream(&mut raw, 1).await;
    raw.send(&Message::Cancel { stream }).await;

    // Frames are handled in order, so once this round trip is answered the cancel has taken effect.
    raw.send(&request(2, service::INFO, Value::Null)).await;
    answer_to(&mut raw, 2).await;

    running.events().publish(ChangeEvent {
        topic: events::TOPIC_PROJECT_ADDED.to_owned(),
        id: "project-1".to_owned(),
    });
    raw.send(&request(3, service::INFO, Value::Null)).await;
    let mut events_after_cancel = 0;
    loop {
        match raw.recv().await {
            Some(Message::Response { id: 3, .. }) => break,
            Some(Message::Event { stream: seen, .. }) if seen == stream => events_after_cancel += 1,
            Some(_) => {}
            None => panic!("the connection closed early"),
        }
    }
    assert_eq!(events_after_cancel, 0, "a cancelled stream sends no events");
    stop(running).await;
}

#[tokio::test]
async fn shutting_down_ends_open_streams_with_a_reason() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let mut subscription = client
        .subscribe_events()
        .await
        .expect("events.subscribe answers");

    let stopper = connect(home.path(), "default").await;
    stopper
        .shutdown()
        .await
        .expect("service.shutdown is answered");

    match timeout(super::common::REPLY_WAIT, subscription.next())
        .await
        .expect("the stream ends")
    {
        Some(Err(error)) => assert_eq!(error.code, codes::SERVICE_SHUTTING_DOWN),
        other => panic!("expected the shutdown reason, got {other:?}"),
    }
    assert!(
        subscription.next().await.is_none(),
        "nothing follows the end"
    );
    timeout(Duration::from_secs(10), running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

/// A subscriber that does not keep up ends with `stream.overflow` once its queue is full, instead of building an
/// unbounded backlog. Nothing follows the error.
#[tokio::test]
async fn a_subscriber_that_falls_behind_ends_with_stream_overflow() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let mut subscription = client
        .subscribe_events()
        .await
        .expect("events.subscribe answers");

    for number in 0..2_000 {
        running.events().publish(ChangeEvent {
            topic: events::TOPIC_PROJECT_ADDED.to_owned(),
            id: number.to_string(),
        });
    }
    // Nothing reads the subscription while the events arrive, so they queue up in the client until its limit is hit.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let overflow = timeout(Duration::from_secs(10), async {
        loop {
            match subscription.next().await {
                Some(Ok(_)) => {}
                Some(Err(error)) => break error,
                None => panic!("the stream ended without saying why"),
            }
        }
    })
    .await
    .expect("the overflow arrives in time");
    assert_eq!(overflow.code, codes::STREAM_OVERFLOW);
    assert!(
        subscription.next().await.is_none(),
        "nothing follows the error"
    );
    stop(running).await;
}

/// Reads frames until the response to `id` arrives and returns the stream id it carries.
async fn subscribed_stream(raw: &mut Raw, id: u64) -> u64 {
    match answer_to(raw, id).await {
        Message::Response {
            result: Some(value),
            error: None,
            ..
        } => value["stream"].as_u64().expect("the answer names a stream"),
        other => panic!("expected the subscription answer, got {other:?}"),
    }
}

/// Reads frames until the response to `id` arrives.
async fn answer_to(raw: &mut Raw, id: u64) -> Message {
    loop {
        let message = raw.recv().await.expect("the connection stays open");
        if matches!(&message, Message::Response { id: answered, .. } if *answered == id) {
            return message;
        }
    }
}
