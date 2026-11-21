//! Concurrency on one connection: many requests at once, and a slow request that does not hold up the others.

use std::sync::Arc;
use std::time::Duration;

use massorca_protocol::ErrorBody;
use massorca_server::{Owner, Permission, RequestCtx};
use serde_json::{Value, json};
use tokio::sync::Notify;
use tokio::time::timeout;

use super::common::{connect, start, stop, temp_home};

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_slow_request_does_not_hold_up_the_others() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let demo = Owner::Extension("demo".to_owned());
    let gate = Arc::new(Notify::new());
    let _capability = running
        .registries()
        .capabilities
        .register("demo/1", demo.clone())
        .expect("the capability registers");
    let held = Arc::clone(&gate);
    let _gated = running
        .registries()
        .methods
        .register(
            demo,
            "ext.demo.gate",
            "demo/1",
            Permission::Read,
            move |_ctx: RequestCtx, _params: Value| {
                let held = Arc::clone(&held);
                async move {
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

    // The gate is still shut, yet other requests on the same connection complete.
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
