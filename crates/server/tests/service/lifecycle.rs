//! Service lifecycle: one service per profile, independent profiles, and a shutdown that frees the profile.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    // A pipe with a connected client cannot be taken over, so the client goes first, as a CLI would.
    drop(client);
    // Windows releases the pipe a moment after the accept loop ends, so the restart retries until the profile is free.
    let restarted = timeout(Duration::from_secs(5), async {
        loop {
            match Service::start(service_config(home.path(), "default")).await {
                Ok(running) => break running,
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
    })
    .await
    .expect("the profile is freed in time");
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

#[cfg(unix)]
#[tokio::test]
async fn the_profile_directory_and_socket_are_private_to_their_owner() {
    use std::os::unix::fs::PermissionsExt;

    use super::common::{profile, start, stop, temp_home};

    let home = temp_home();
    let running = start(home.path(), "default").await;
    let profile = profile(home.path(), "default");

    let dir_mode = std::fs::metadata(profile.dir())
        .expect("the profile directory")
        .permissions()
        .mode();
    let socket_mode = std::fs::metadata(profile.socket_path())
        .expect("the socket file")
        .permissions()
        .mode();
    assert_eq!(dir_mode & 0o777, 0o700);
    assert_eq!(socket_mode & 0o777, 0o600);
    stop(running).await;
}

#[cfg(windows)]
#[tokio::test]
async fn a_squatter_on_the_endpoint_name_stops_the_service_from_starting() {
    use interprocess::local_socket::ListenerOptions;
    use massorca_profile::{ensure_dir, local_socket_name};
    use massorca_server::Service;

    use super::common::{profile, service_config, temp_home};

    let home = temp_home();
    let profile = profile(home.path(), "default");
    ensure_dir(&profile).expect("the profile directory exists");
    let _squatter = ListenerOptions::new()
        .name(local_socket_name(&profile).expect("the endpoint name"))
        .create_tokio()
        .expect("the squatter listens");

    let started = Service::start(service_config(home.path(), "default")).await;
    assert!(
        started.is_err(),
        "a service must not start beside a listener that already holds the endpoint name"
    );
}

/// Sets its flag when dropped; a handler future is dropped only when it finishes or the runtime cancels it.
struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn a_handler_still_running_at_the_grace_limit_is_dropped_before_the_profile_is_released() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let demo = Owner::Extension("demo".to_owned());
    let _capability = running
        .registries()
        .capabilities
        .register("demo/1", demo.clone())
        .expect("the capability registers");
    let entered = Arc::new(Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let (started, flag) = (Arc::clone(&entered), Arc::clone(&dropped));
    let _stuck = running
        .registries()
        .methods
        .register(
            demo,
            "ext.demo.stuck",
            "demo/1",
            Permission::Read,
            move |_ctx: RequestCtx, _params: Value| {
                let (started, flag) = (Arc::clone(&started), Arc::clone(&flag));
                async move {
                    let _sentinel = DropFlag(flag);
                    started.notify_one();
                    std::future::pending::<()>().await;
                    Ok::<Value, ErrorBody>(Value::Null)
                }
            },
        )
        .expect("the method registers");

    let client = connect(home.path(), "default").await;
    let call = tokio::spawn(async move {
        client
            .call::<_, Value>("ext.demo.stuck", &Value::Null)
            .await
    });
    timeout(Duration::from_secs(5), entered.notified())
        .await
        .expect("the handler started");

    running.shutdown_handle().shutdown();
    timeout(Duration::from_secs(20), running.wait())
        .await
        .expect("the service stops after its grace period")
        .expect("the service stops cleanly");
    assert!(
        dropped.load(Ordering::SeqCst),
        "the handler was dropped before the profile was released, not left running"
    );
    // The call ends with an error once its connection is gone.
    let _ = call.await;
}
