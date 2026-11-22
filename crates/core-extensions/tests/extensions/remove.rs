//! Enable, disable and remove, with guards, cleanups and the CLI uninstall.

use std::sync::{Arc, Mutex};

use massorca_core_extensions::removal::CliAction;
use massorca_core_extensions::{HookError, codes};

use crate::fakes::FakeHost;
use crate::fixture::agent_package;
use crate::service::{Setup, setup};

fn installed(host: FakeHost, run_installer: bool) -> Setup {
    let s = setup(host);
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&source, run_installer).expect("installs");
    s
}

fn busy(_: &str) -> Result<(), HookError> {
    Err(HookError::new("agent.busy", "a kit session is live"))
}

#[test]
fn disabling_asks_the_guards_and_keeps_the_files() {
    let s = installed(FakeHost::new().with_program("kit"), false);
    let refusing = s.service.register_remove_guard("agents", busy);
    let error = s.service.set_enabled("acme.kit", false).expect_err("busy");
    assert_eq!(error.code(), "agent.busy");
    assert_eq!(s.lock_json().expect("lock")["packages"][0]["enabled"], true);
    drop(refusing);

    let outcome = s.service.set_enabled("acme.kit", false).expect("disables");
    assert!(outcome.changed);
    assert_eq!(
        s.lock_json().expect("lock")["packages"][0]["enabled"],
        false
    );
    assert!(s.exists("pkgs/acme.kit"));
    assert!(
        !s.service
            .set_enabled("acme.kit", false)
            .expect("again")
            .changed
    );
    assert!(
        s.service
            .set_enabled("acme.kit", true)
            .expect("enables")
            .changed
    );
    let unknown = s
        .service
        .set_enabled("acme.none", true)
        .expect_err("unknown");
    assert_eq!(unknown.code(), codes::NOT_INSTALLED);
}

#[test]
fn a_guard_refusal_stops_the_removal() {
    let s = installed(FakeHost::new().with_program("kit"), false);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = seen.clone();
    let _guard = s.service.register_remove_guard("agents", move |id| {
        record.lock().expect("seen").push(id.to_owned());
        busy(id)
    });
    let error = s
        .service
        .remove("acme.kit", true, true)
        .expect_err("refused");
    assert_eq!(error.code(), "agent.busy");
    assert_eq!(*seen.lock().expect("seen"), vec!["acme.kit".to_owned()]);
    assert_eq!(
        s.lock_json().expect("lock")["packages"][0]["id"],
        "acme.kit"
    );
    assert!(s.exists("pkgs/acme.kit"));
}

#[test]
fn removal_deletes_the_package_and_its_data() {
    let s = installed(FakeHost::new(), true);
    let removed = s.service.remove("acme.kit", false, false).expect("removes");
    assert_eq!(removed.id, "acme.kit");
    assert!(removed.failures.is_empty());
    assert_eq!(
        s.lock_json().expect("lock")["packages"],
        serde_json::json!([])
    );
    assert!(!s.exists("pkgs/acme.kit"));
    assert!(!s.exists("data/acme.kit"));
    let again = s
        .service
        .remove("acme.kit", false, false)
        .expect_err("gone");
    assert_eq!(again.code(), codes::NOT_INSTALLED);
}

#[test]
fn cleanups_run_only_when_asked_and_their_failures_are_reported() {
    let calls = Arc::new(Mutex::new(0));
    for delete_agent_homes in [false, true] {
        let s = installed(FakeHost::new().with_program("kit"), false);
        let counter = calls.clone();
        let _cleanup = s.service.register_remove_cleanup("agents", move |_| {
            *counter.lock().expect("calls") += 1;
            Err(HookError::new("agent.home_locked", "a file is in use"))
        });
        let removed = s
            .service
            .remove("acme.kit", delete_agent_homes, false)
            .expect("removes despite the cleanup");
        assert_eq!(
            s.lock_json().expect("lock")["packages"],
            serde_json::json!([])
        );
        if delete_agent_homes {
            assert_eq!(removed.failures.len(), 1);
            assert_eq!(removed.failures[0].owner, "agents");
            assert_eq!(removed.failures[0].code, "agent.home_locked");
        } else {
            assert!(removed.failures.is_empty());
        }
    }
    assert_eq!(*calls.lock().expect("calls"), 1);
}

#[test]
fn the_cli_uninstall_runs_only_for_a_cli_the_package_installed_and_only_when_asked() {
    let s = installed(FakeHost::new(), true);
    let offered = s.service.remove("acme.kit", false, false).expect("removes");
    let command = vec!["kit-setup".to_owned(), "remove".to_owned()];
    assert_eq!(
        offered.cli[0].action,
        CliAction::Offered {
            command: command.clone()
        }
    );

    let s = installed(FakeHost::new(), true);
    let ran = s.service.remove("acme.kit", false, true).expect("removes");
    assert_eq!(
        ran.cli[0].action,
        CliAction::Ran {
            command: command.clone(),
            exit_code: Some(0)
        }
    );
    assert_eq!(s.host.runs().last(), Some(&command));
    assert!(s.service.layout().uninstall_log("acme.kit").exists());

    let s = installed(FakeHost::new().with_program("kit"), false);
    let left = s.service.remove("acme.kit", false, true).expect("removes");
    assert_eq!(left.cli[0].action, CliAction::LeftAlone);
    assert!(s.host.runs().is_empty());
}

#[test]
fn a_failed_cli_uninstall_is_reported_after_the_removal() {
    let s = installed(FakeHost::new().exit_for("remove", 2), true);
    let removed = s.service.remove("acme.kit", false, true).expect("removes");
    assert!(matches!(removed.cli[0].action, CliAction::Failed { .. }));
    assert_eq!(
        s.lock_json().expect("lock")["packages"],
        serde_json::json!([])
    );
}
