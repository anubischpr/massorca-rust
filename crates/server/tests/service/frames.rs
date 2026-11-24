//! Frame limits: an oversized reply fails only its own request, an oversized item ends only its stream, and an oversized
//! request never reaches the wire.

use std::time::Duration;

use massorca_client::ClientError;
use massorca_protocol::methods::events::ChangeEvent;
use massorca_protocol::methods::service;
use massorca_protocol::{ErrorBody, MAX_FRAME_LEN_LOCAL, codes};
use massorca_server::{Owner, Permission, RequestCtx};
use serde_json::{Value, json};
use tokio::time::timeout;

use super::common::{connect, remote_code, start, stop, temp_home};

/// Text whose JSON encoding is larger than one frame may be.
fn oversized_text() -> String {
    "x".repeat(MAX_FRAME_LEN_LOCAL as usize + 1024)
}

#[tokio::test]
async fn an_oversized_reply_fails_only_its_own_request() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let demo = Owner::Extension("demo".to_owned());
    let _capability = running
        .registries()
        .capabilities
        .register("demo/1", demo.clone())
        .expect("the capability registers");
    let _huge = running
        .registries()
        .methods
        .register(
            demo,
            "ext.demo.huge",
            "demo/1",
            Permission::Read,
            |_ctx: RequestCtx, _params: Value| async {
                Ok::<Value, ErrorBody>(json!(oversized_text()))
            },
        )
        .expect("the method registers");

    let client = connect(home.path(), "default").await;
    let error = timeout(
        Duration::from_secs(10),
        client.call::<_, Value>("ext.demo.huge", &Value::Null),
    )
    .await
    .expect("the refusal arrives in time")
    .expect_err("the reply is larger than a frame");
    assert_eq!(remote_code(&error), codes::RESPONSE_TOO_LARGE);

    let info = client
        .service_info()
        .await
        .expect("the connection still serves");
    assert_eq!(info.profile, "default");
    stop(running).await;
}

#[tokio::test]
async fn an_oversized_stream_item_ends_its_stream_and_keeps_the_connection() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let mut events = client
        .subscribe_events()
        .await
        .expect("the subscription opens");

    running.events().publish(ChangeEvent {
        topic: "project.added".to_owned(),
        id: oversized_text(),
    });
    let item = timeout(super::common::REPLY_WAIT, events.next())
        .await
        .expect("the stream reports in time")
        .expect("the stream ends with an error")
        .expect_err("the item is larger than a frame");
    assert_eq!(item.code, codes::STREAM_ITEM_TOO_LARGE);

    let info = client
        .service_info()
        .await
        .expect("the connection still serves");
    assert_eq!(info.profile, "default");
    stop(running).await;
}

#[tokio::test]
async fn an_oversized_request_fails_locally_and_keeps_the_connection() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let error = client
        .call::<_, Value>(service::INFO, &json!(oversized_text()))
        .await
        .expect_err("the request is larger than a frame");
    assert!(
        matches!(error, ClientError::Io(_)),
        "refused before it is sent: {error:?}"
    );

    let info = client
        .service_info()
        .await
        .expect("the connection still serves");
    assert_eq!(info.profile, "default");
    stop(running).await;
}
