//! Concurrency on one connection: requests overlap, and a slow request does not hold up the others.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use massorca_protocol::ErrorBody;
use massorca_server::{Owner, Permission, RequestCtx};
use serde_json::{Value, json};
use tokio::sync::{Barrier, Notify, Semaphore};
use tokio::time::timeout;

use super::common::{connect, register_stuck, start, stop, temp_home};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn many_requests_share_one_connection() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let calls: Vec<_> = (0..32)
        .map(|_| {
            let client = client.clone();
            tokio::spawn(async move { client.service_info().await.map(|info| info.profile) })
        })
        .collect();
    for call in calls {
        let profile = call
            .await
            .expect("the task finishes")
            .expect("service.info answers");
        assert_eq!(profile, "default");
    }
    stop(running).await;
}

/// Every call waits at a barrier for the others, so the test completes only if they all run at once. A service that
/// ran requests one after another would hold the first call at the barrier forever.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn requests_on_one_connection_run_at_the_same_time() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let demo = Owner::Extension("demo".to_owned());
    let _capability = running
        .registries()
        .capabilities
        .register("demo/1", demo.clone())
        .expect("the capability registers");
    const PARTIES: usize = 4;
    let meeting = Arc::new(Barrier::new(PARTIES));
    let _rendezvous = running
        .registries()
        .methods
        .register(
            demo,
            "ext.demo.rendezvous",
            "demo/1",
            Permission::Read,
            move |_ctx: RequestCtx, _params: Value| {
                let meeting = Arc::clone(&meeting);
                async move {
                    meeting.wait().await;
                    Ok::<Value, ErrorBody>(json!("met"))
                }
            },
        )
        .expect("the method registers");

    let client = connect(home.path(), "default").await;
    let calls: Vec<_> = (0..PARTIES)
        .map(|_| {
            let client = client.clone();
            tokio::spawn(async move {
                client
                    .call::<_, Value>("ext.demo.rendezvous", &Value::Null)
                    .await
            })
        })
        .collect();
    for call in calls {
        let met = timeout(Duration::from_secs(5), call)
            .await
            .expect("the calls reached the barrier together")
            .expect("the task finishes")
            .expect("the call completes");
        assert_eq!(met, json!("met"));
    }
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_slow_request_does_not_hold_up_the_others() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let demo = Owner::Extension("demo".to_owned());
    let gate = Arc::new(Notify::new());
    let entered = Arc::new(Notify::new());
    let _capability = running
        .registries()
        .capabilities
        .register("demo/1", demo.clone())
        .expect("the capability registers");
    let held = Arc::clone(&gate);
    let started = Arc::clone(&entered);
    let _gated = running
        .registries()
        .methods
        .register(
            demo,
            "ext.demo.gate",
            "demo/1",
            Permission::Read,
            move |_ctx: RequestCtx, _params: Value| {
                let (held, started) = (Arc::clone(&held), Arc::clone(&started));
                async move {
                    started.notify_one();
                    held.notified().await;
                    Ok::<Value, ErrorBody>(json!("released"))
                }
            },
        )
        .expect("the method registers");

    let client = connect(home.path(), "default").await;
    let slow_client = client.clone();
    let slow = tokio::spawn(async move {
        slow_client
            .call::<_, Value>("ext.demo.gate", &Value::Null)
            .await
    });
    // The gated request has started before the other one is sent, so the other one can only overtake it.
    timeout(Duration::from_secs(5), entered.notified())
        .await
        .expect("the gated request started");

    let info = timeout(Duration::from_secs(5), client.service_info())
        .await
        .expect("the other request is not held up")
        .expect("service.info answers");
    assert_eq!(info.profile, "default");
    assert!(!slow.is_finished(), "the gated request is still waiting");

    gate.notify_one();
    let released = slow
        .await
        .expect("the task finishes")
        .expect("the gated call completes");
    assert_eq!(released, json!("released"));
    stop(running).await;
}

/// A connection reads a new request only while fewer than 64 are in flight, so 65 calls leave one of them waiting.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_connection_holds_at_most_64_requests_in_flight() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let demo = Owner::Extension("demo".to_owned());
    let _capability = running
        .registries()
        .capabilities
        .register("demo/1", demo.clone())
        .expect("the capability registers");
    let entered = Arc::new(AtomicUsize::new(0));
    // Each handler takes one permit before it finishes, so the test decides how many may finish.
    let gate = Arc::new(Semaphore::new(0));
    let (count, release) = (Arc::clone(&entered), Arc::clone(&gate));
    let _counted = running
        .registries()
        .methods
        .register(
            demo,
            "ext.demo.counted",
            "demo/1",
            Permission::Read,
            move |_ctx: RequestCtx, _params: Value| {
                let (count, release) = (Arc::clone(&count), Arc::clone(&release));
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    release
                        .acquire()
                        .await
                        .expect("the gate stays open")
                        .forget();
                    Ok::<Value, ErrorBody>(json!("done"))
                }
            },
        )
        .expect("the method registers");

    let client = connect(home.path(), "default").await;
    let calls: Vec<_> = (0..65)
        .map(|_| {
            let client = client.clone();
            tokio::spawn(async move {
                client
                    .call::<_, Value>("ext.demo.counted", &Value::Null)
                    .await
            })
        })
        .collect();

    wait_for_count(&entered, 64).await;
    // Time for a 65th request to be read, if the connection were willing to read it.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        entered.load(Ordering::SeqCst),
        64,
        "the 65th request waits for a free slot"
    );

    gate.add_permits(1);
    wait_for_count(&entered, 65).await;
    gate.add_permits(64);
    for call in calls {
        timeout(Duration::from_secs(5), call)
            .await
            .expect("the call completes in time")
            .expect("the task finishes")
            .expect("the call succeeds");
    }
    stop(running).await;
}

async fn wait_for_count(count: &AtomicUsize, expected: usize) {
    timeout(Duration::from_secs(5), async {
        while count.load(Ordering::SeqCst) < expected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("the count did not reach {expected}"));
}

/// A client that disconnects takes its requests with it: their handlers are cancelled as soon as the service sees the
/// disconnect, not when the service shuts down.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_disconnected_client_cancels_the_requests_it_left_in_flight() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let entered = Arc::new(Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let _stuck = register_stuck(&running, Arc::clone(&entered), Arc::clone(&dropped));

    let client = connect(home.path(), "default").await;
    let call = tokio::spawn(async move {
        client
            .call::<_, Value>("ext.demo.stuck", &Value::Null)
            .await
    });
    timeout(Duration::from_secs(5), entered.notified())
        .await
        .expect("the call reached its handler");

    // Aborting the task drops its client, which closes the connection.
    call.abort();
    timeout(Duration::from_secs(5), async {
        while !dropped.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the handler is cancelled once the service sees the disconnect");
    stop(running).await;
}

/// Calls beyond what the client's queues hold wait their turn; every one of them is answered.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_burst_of_calls_larger_than_the_queues_completes_every_call() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let calls: Vec<_> = (0..600)
        .map(|_| {
            let client = client.clone();
            tokio::spawn(async move { client.service_info().await.map(|info| info.profile) })
        })
        .collect();
    for call in calls {
        let profile = timeout(Duration::from_secs(20), call)
            .await
            .expect("the burst drains in time")
            .expect("the task finishes")
            .expect("service.info answers");
        assert_eq!(profile, "default");
    }
    stop(running).await;
}
