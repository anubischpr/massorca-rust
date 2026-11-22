//! A resilient stream outlives a service restart: the loss reads as unverifiable, the return as live, and items flow
//! again on the new connection.

use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use massorca_client::{
    Backoff, Client, ClientError, Link, Reopen, Resilient, ResilientSubscription, Subscription,
};
use massorca_protocol::methods::events::{self, ChangeEvent};
use massorca_server::RunningService;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};

use super::common::{client_options, profile, start, stop, temp_home};

struct Events {
    home: PathBuf,
}

impl Reopen for Events {
    fn connect(&mut self) -> impl Future<Output = Result<Client, ClientError>> + Send {
        let profile = profile(&self.home, "default");
        async move { Client::connect(&profile, client_options()).await }
    }

    fn open(
        &mut self,
        client: &Client,
    ) -> impl Future<Output = Result<Subscription, ClientError>> + Send {
        client.subscribe_events()
    }
}

type Received = Option<Result<Resilient, ClientError>>;

/// Publishes an event every 50 ms until the stream yields an item, so the test does not race the subscription.
async fn until_item(received: &mut mpsc::UnboundedReceiver<Received>, running: &RunningService) {
    let bus = running.events().clone();
    let publisher = tokio::spawn(async move {
        loop {
            bus.publish(ChangeEvent {
                topic: events::TOPIC_PROJECT_ADDED.to_owned(),
                id: "project-1".to_owned(),
            });
            sleep(Duration::from_millis(50)).await;
        }
    });
    let item = timeout(Duration::from_secs(10), received.recv())
        .await
        .expect("an item in time")
        .expect("the reader is running");
    publisher.abort();
    assert!(
        matches!(item, Some(Ok(Resilient::Item(_)))),
        "expected an item, got {item:?}"
    );
}

async fn next(received: &mut mpsc::UnboundedReceiver<Received>) -> Received {
    timeout(Duration::from_secs(10), received.recv())
        .await
        .expect("the stream reports in time")
        .expect("the reader is running")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stream_reconnects_after_a_restart_and_reports_the_gap() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let mut stream = ResilientSubscription::new(
        Events {
            home: home.path().to_path_buf(),
        },
        Backoff::new(Duration::from_millis(20), Duration::from_millis(100)),
    );
    let (sender, mut received) = mpsc::unbounded_channel();
    let reader = tokio::spawn(async move {
        loop {
            let item = stream.next().await;
            let done = !matches!(item, Some(Ok(_)));
            if sender.send(item).is_err() || done {
                return;
            }
        }
    });
    until_item(&mut received, &running).await;

    stop(running).await;
    // Items published before the stop may still be queued ahead of the loss.
    let first_loss = loop {
        match next(&mut received).await {
            Some(Ok(Resilient::Item(_))) => {}
            Some(Ok(Resilient::Link(link))) => break link,
            other => panic!("expected the loss, got {other:?}"),
        }
    };
    let Link::Unverifiable { attempt: 1, .. } = first_loss else {
        panic!("the loss is unverifiable, attempt 1: {first_loss:?}");
    };

    let running = start(home.path(), "default").await;
    let mut last_attempt = 1;
    loop {
        match next(&mut received).await {
            Some(Ok(Resilient::Link(Link::Unverifiable { attempt, .. }))) => {
                assert!(attempt > last_attempt, "attempts count up");
                last_attempt = attempt;
            }
            Some(Ok(Resilient::Link(Link::Live))) => break,
            other => panic!("expected the return, got {other:?}"),
        }
    }
    until_item(&mut received, &running).await;

    reader.abort();
    stop(running).await;
}
