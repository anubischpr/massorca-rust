//! Capabilities: dropping a registration withdraws the capability from later handshakes and its methods stop working.

use massorca_protocol::codes;
use massorca_server::{Owner, Permission};
use serde_json::{Value, json};

use super::common::{connect, echo, remote_code, start, stop, temp_home};

#[tokio::test]
async fn dropping_a_capability_handle_withdraws_it_from_later_handshakes() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let demo = Owner::Extension("demo".to_owned());
    let capability = running
        .registries()
        .capabilities
        .register("demo/1", demo.clone())
        .expect("the capability registers");
    let _echo = running
        .registries()
        .methods
        .register(demo, "ext.demo.echo", "demo/1", Permission::Read, echo)
        .expect("the method registers");

    let offered = connect(home.path(), "default").await;
    assert!(offered.has_capability("demo/1"));
    let answer: Value = offered
        .call("ext.demo.echo", &json!({"hello": "world"}))
        .await
        .expect("the method works while its capability is offered");
    assert_eq!(answer, json!({"hello": "world"}));

    drop(capability);
    let withdrawn = connect(home.path(), "default").await;
    assert!(
        !withdrawn.has_capability("demo/1"),
        "the next welcome no longer lists it"
    );
    let error = withdrawn
        .call::<_, Value>("ext.demo.echo", &json!({}))
        .await
        .expect_err("a method of a withdrawn capability is not negotiated");
    assert_eq!(remote_code(&error), codes::CAPABILITY_NOT_NEGOTIATED);
    stop(running).await;
}
