//! Service lifecycle: one service per profile, independent profiles, and a shutdown that frees the profile.

use std::sync::Arc;
use std::time::Duration;

use massorca_protocol::{ErrorBody, PROTOCOL_VERSION};
use massorca_server::{Owner, Permission, RequestCtx, Service, ServiceError};
use serde_json::{Value, json};
use tokio::sync::Notify;
use tokio::time::timeout;

use super::common::{
    SERVER_VERSION, connect, profile, service_config, service_json, start, stop, temp_home,
};

#[tokio::test]
async fn a_second_service_for_the_same_profile_is_refused() {
    let home = temp_home();
    let first = start(home.path(), "default").await;

    let second = Service::start(service_config(home.path(), "default")).await;
    assert!(
        matches!(second, Err(ServiceError::AlreadyRunning)),
        "the profile is already taken"
    );
    stop(first).await;
}

#[tokio::test]
async fn two_profiles_in_one_home_run_side_by_side() {
    let home = temp_home();
    let alpha = start(home.path(), "alpha").await;
    let beta = start(home.path(), "beta").await;
    let alpha_client = connect(home.path(), "alpha").await;
    let beta_client = connect(home.path(), "beta").await;
    assert_eq!(
        alpha_client
            .service_info()
            .await
            .expect("alpha answers")
            .profile,
        "alpha"
    );
    assert_eq!(
        beta_client
            .service_info()
            .await
            .expect("beta answers")
            .profile,
        "beta"
    );

    alpha_client
        .shutdown()
        .await
        .expect("alpha stops on request");
    timeout(Duration::from_secs(10), alpha.wait())
        .await
        .expect("alpha exits in time")
        .expect("alpha exits cleanly");
    assert_eq!(
        beta_client
            .service_info()
            .await
            .expect("beta keeps serving")
            .profile,
        "beta"
    );
    stop(beta).await;
}

#[tokio::test]
async fn a_shutdown_request_stops_the_service_and_frees_the_profile() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    client
        .shutdown()
        .await
        .expect("the request is answered before the service stops");
    timeout(Duration::from_secs(10), running.wait())
        .await
        .expect("wait returns in time")
        .expect("the service stopped cleanly");
    let restarted = start(home.path(), "default").await;
    stop(restarted).await;
}

#[tokio::test]
async fn requests_in_flight_finish_before_the_service_stops() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let demo = Owner::Extension("demo".to_owned());
    let entered = Arc::new(Notify::new());
    let leave = Arc::new(Notify::new());
    let _capability = running
        .registries()
        .capabilities
        .register("demo/1", demo.clone())
        .expect("the capability registers");
    let (started, released) = (Arc::clone(&entered), Arc::clone(&leave));
    let _hold = running
        .registries()
        .methods
        .register(
            demo,
            "ext.demo.hold",
            "demo/1",
            Permission::Read,
            move |_ctx: RequestCtx, _params: Value| {
                let (started, released) = (Arc::clone(&started), Arc::clone(&released));
                async move {
                    started.notify_one();
                    released.notified().await;
                    Ok::<Value, ErrorBody>(json!("finished"))
                }
            },
        )
        .expect("the method registers");

    let held_client = connect(home.path(), "default").await;
    let held = tokio::spawn(async move {
        held_client
            .call::<_, Value>("ext.demo.hold", &Value::Null)
            .await
    });
    timeout(Duration::from_secs(5), entered.notified())
        .await
        .expect("the held request started");

    let stopper = connect(home.path(), "default").await;
    stopper
        .shutdown()
        .await
        .expect("shutdown is accepted while a request is in flight");
    leave.notify_one();

    let finished = held
        .await
        .expect("the task finishes")
        .expect("the held request still answers");
    assert_eq!(finished, json!("finished"));
    timeout(Duration::from_secs(10), running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

#[tokio::test]
async fn dropping_a_running_service_stops_it() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    drop(running);

    // Dropping requests shutdown; the profile frees once the service has stopped, so a new start may wait a moment.
    let restarted = timeout(Duration::from_secs(10), async {
        loop {
            match Service::start(service_config(home.path(), "default")).await {
                Ok(running) => break running,
                Err(ServiceError::AlreadyRunning) => {
                    tokio::time::sleep(Duration::from_millis(25)).await
                }
                Err(other) => panic!("unexpected start failure: {other}"),
            }
        }
    })
    .await
    .expect("the profile frees after the drop");
    stop(restarted).await;
}

#[tokio::test]
async fn service_json_describes_the_service_and_stays_after_shutdown() {
    let home = temp_home();
    let running = start(home.path(), "default").await;

    let described = service_json(home.path(), "default");
    assert_eq!(described["version"], SERVER_VERSION);
    assert_eq!(described["pid"], std::process::id());
    assert_eq!(described["protocol"]["min"], 1);
    assert_eq!(described["protocol"]["max"], PROTOCOL_VERSION);
    assert_eq!(described["profile"], "default");
    assert!(
        described["started_at_ms"]
            .as_i64()
            .is_some_and(|started| started > 0)
    );

    stop(running).await;
    assert!(
        profile(home.path(), "default").info_path().exists(),
        "service.json stays in place after shutdown"
    );
}
