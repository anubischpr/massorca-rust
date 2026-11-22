//! The change lock is released while a vendor command runs: other packages change meanwhile, and the installing package
//! is busy until the install is committed (0062, second review).

use std::sync::Arc;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use massorca_core_extensions::source::Source;
use massorca_core_extensions::{Extensions, codes};
use massorca_profile::Profile;

use crate::fakes::{FakeHost, Gate};
use crate::fixture::{agent_package, package_files, write_dir};

const WAIT: Duration = Duration::from_secs(20);

#[test]
fn other_packages_change_while_an_install_runs_its_vendor_command() {
    let home = tempfile::tempdir().expect("temp home");
    let profile = Profile::resolve_with(
        Some(home.path().to_path_buf()),
        Some("test".to_owned()),
        |_| None,
    )
    .expect("profile");
    let host = Arc::new(FakeHost::new().with_program("other"));
    let (gate, started, release) = Gate::new(host.clone());
    let service = Arc::new(Extensions::new(&profile, host.clone(), Arc::new(gate)));

    let other = home.path().join("other");
    write_dir(&other, &package_files(&named_package("other")));
    let other = Source::Dir(other);
    let other_consent = service.inspect(&other).expect("inspect").consent;
    service
        .install(&other, Some(&other_consent), false, None)
        .expect("the detected package installs");

    let slow = home.path().join("slow");
    write_dir(&slow, &package_files(&named_package("slow")));
    let slow = Source::Dir(slow);
    let slow_consent = service.inspect(&slow).expect("inspect").consent;
    let installing = {
        let (service, slow, consent) = (service.clone(), slow.clone(), slow_consent.clone());
        thread::spawn(move || service.install(&slow, Some(&consent), true, None))
    };
    recv_timeout_or_fail(&started, "the vendor command starts");

    // While the vendor command runs, another package is disabled by a second thread.
    let (done_tx, done_rx) = channel();
    {
        let service = service.clone();
        thread::spawn(move || {
            let _ = done_tx.send(service.set_enabled("acme.other", false));
        });
    }
    let disabled = recv_timeout_or_fail(&done_rx, "a disable completes while an install runs");
    assert!(disabled.expect("disabled").changed);

    // The installing package refuses a second install and a removal with extension.busy.
    let second = service.install(&slow, Some(&slow_consent), true, None);
    assert_eq!(second.expect_err("busy").code(), codes::BUSY);
    assert_eq!(
        service
            .remove("acme.slow", false, false)
            .expect_err("busy")
            .code(),
        codes::BUSY
    );

    release.send(()).expect("release the vendor command");
    installing
        .join()
        .expect("the install thread")
        .expect("the install commits");

    // Committed: the mark is gone, so the removal now goes through.
    let removed = service.remove("acme.slow", false, false).expect("removed");
    assert_eq!(removed.id, "acme.slow");
}

fn recv_timeout_or_fail<T>(receiver: &std::sync::mpsc::Receiver<T>, what: &str) -> T {
    receiver
        .recv_timeout(WAIT)
        .unwrap_or_else(|_| panic!("{what} within {WAIT:?}"))
}

/// A one-CLI agent package with its own id, `acme.<name>`, whose CLI is `<name>`.
fn named_package(name: &str) -> serde_json::Value {
    let mut manifest = agent_package("acme", name, name);
    manifest["id"] = serde_json::json!(format!("acme.{name}"));
    manifest
}
