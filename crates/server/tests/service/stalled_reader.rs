//! A client that stops reading must not hold up the service's stop, nor the closes of other connections in the process.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use massorca_server::{CapabilityHandle, Owner, Permission, RegistrationHandle, RunningService};
use serde_json::json;
use tokio::time::timeout;

use super::common::{Raw, echo, hello, request, start, temp_home};

/// Replies large enough to fill the pipe and the writer's buffer many times over.
const FLOOD_REQUESTS: u64 = 48;
const FLOOD_REPLY_BYTES: usize = 128 * 1024;
/// How long a stop or a close may take; the shutdown grace is 5 seconds.
const CLOSE_WAIT: Duration = Duration::from_secs(15);
/// The whole test, runtime drop included, so a hang fails instead of stalling the suite.
const TEST_DEADLINE: Duration = Duration::from_secs(120);

#[test]
fn a_client_that_never_reads_does_not_delay_other_connections_closing() {
    within_deadline(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("a runtime");
        runtime.block_on(async {
            let home = temp_home();

            // A client that never reads, on a service that then stops: its connection is closed under it.
            let (first, _first_echo) = start_with_echo(home.path(), "first").await;
            let stalled = flooded(home.path(), "first").await;
            stop_within(first).await;

            // A client that reads late, on another service: it must get what was written before the close, then the close.
            let (second, _second_echo) = start_with_echo(home.path(), "second").await;
            let mut late = flooded(home.path(), "second").await;
            stop_within(second).await;
            let replies = timeout(CLOSE_WAIT, async {
                let mut replies = 0;
                while late.recv().await.is_some() {
                    replies += 1;
                }
                replies
            })
            .await
            .expect(
                "the late reader sees its connection close although another client never reads",
            );
            // Replies still queued when the grace ran out are dropped; the one being written must still arrive.
            assert!(replies >= 1, "the reply written before the close arrives");
            drop(stalled);
        });
        drop(runtime);
    });
}

/// Runs `body` on its own thread and fails if it has not returned within `TEST_DEADLINE`.
fn within_deadline(body: impl FnOnce() + Send + 'static) {
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(catch_unwind(AssertUnwindSafe(body)));
    });
    match finished.recv_timeout(TEST_DEADLINE) {
        Ok(Ok(())) => {}
        Ok(Err(panic)) => resume_unwind(panic),
        Err(_) => panic!("the test hung for {TEST_DEADLINE:?}"),
    }
}

async fn start_with_echo(
    home: &Path,
    name: &str,
) -> (RunningService, (CapabilityHandle, RegistrationHandle)) {
    let running = start(home, name).await;
    let demo = Owner::Extension("demo".to_owned());
    let capability = running
        .registries()
        .capabilities
        .register("demo/1", demo.clone())
        .expect("the capability registers");
    let method = running
        .registries()
        .methods
        .register(demo, "ext.demo.echo", "demo/1", Permission::Read, echo)
        .expect("the method registers");
    (running, (capability, method))
}

/// A connection past its handshake whose large replies the service is still writing.
async fn flooded(home: &Path, name: &str) -> Raw {
    let mut raw = Raw::open(home, name).await;
    raw.send(&hello()).await;
    raw.recv_step("the welcome").await.expect("a welcome");
    let payload = "x".repeat(FLOOD_REPLY_BYTES);
    for id in 1..=FLOOD_REQUESTS {
        raw.send(&request(id, "ext.demo.echo", json!(payload)))
            .await;
    }
    raw
}

async fn stop_within(running: RunningService) {
    running.shutdown_handle().shutdown();
    timeout(CLOSE_WAIT, running.wait())
        .await
        .expect("the service stops in time although a client does not read")
        .expect("the service stops cleanly");
}
