//! A start that fails names the tail of the service log in its failure, so a slow or failed start explains itself.

mod harness;

use std::fs;
use std::panic::{self, AssertUnwindSafe};

use harness::Sandbox;

#[test]
fn a_failed_autostart_names_the_service_log_in_its_failure() {
    let sandbox = Sandbox::new();
    // A healthy start first, so the profile has a service log; then a store that cannot be opened makes the next start fail
    // after its log is open.
    sandbox.json_ok("default", &["service", "info"]);
    sandbox.json_ok("default", &["service", "stop"]);
    let store = sandbox.profile_dir("default").join("store.sqlite3");
    fs::write(&store, b"not a database, written by a test\n").expect("the store is replaced");

    let failure = panic::catch_unwind(AssertUnwindSafe(|| {
        sandbox.json_ok("default", &["service", "info"])
    }))
    .expect_err("the autostart fails on the unreadable store");

    let message = panic_text(&*failure);
    assert!(
        message.contains("service log "),
        "the failure names the log file: {message}"
    );
    assert!(
        message.contains("service did not start"),
        "the failure carries the log's lines: {message}"
    );
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
        })
        .unwrap_or_default()
}
