//! Enable, disable and remove, with guards, cleanups and the CLI uninstall.

use std::sync::{Arc, Mutex};

use massorca_core_extensions::host::current_os;
use massorca_core_extensions::removal::CliAction;
use massorca_core_extensions::source::Source;
use massorca_core_extensions::{HookError, codes};
use massorca_ext_package::manifest::agent::CliOs;

use crate::fakes::FakeHost;
use crate::fixture::agent_package;
use crate::service::{Setup, setup};

fn installed(host: FakeHost, run_installer: bool) -> Setup {
    let s = setup(host);
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&source, run_installer).expect("installs");
    s
}

/// The built-in Claude package, installed by its own install command, which the fake host records.
fn installed_claude_with(host: FakeHost) -> Setup {
    let s = setup(host);
    s.install(&Source::Catalogue("massorca.claude".to_owned()), true)
        .expect("installs");
    s
}

fn installed_claude() -> Setup {
    installed_claude_with(FakeHost::new())
}

/// The uninstall command the built-in Claude declares for this host.
fn claude_uninstall() -> Vec<String> {
    let argv: &[&str] = match current_os() {
        Some(CliOs::Windows) => &["winget", "uninstall", "Anthropic.ClaudeCode"],
        Some(CliOs::Macos) => &["brew", "uninstall", "--cask", "claude-code"],
        _ => &["npm", "uninstall", "-g", "-ai/claude-code"],
    };
    argv.iter().map(|part| (*part).to_owned()).collect()
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
        .remove("acme.kit", true, true, None)
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
    let s = installed_claude();
    let removed = s
        .service
        .remove("massorca.claude", false, false, None)
        .expect("removes");
    assert_eq!(removed.id, "massorca.claude");
    assert!(removed.failures.is_empty());
    assert_eq!(
        s.lock_json().expect("lock")["packages"],
        serde_json::json!([])
    );
    assert!(!s.exists("pkgs/massorca.claude"));
    assert!(!s.exists("data/massorca.claude"));
    let again = s
        .service
        .remove("massorca.claude", false, false, None)
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
            .remove("acme.kit", delete_agent_homes, false, None)
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
    let s = installed_claude();
    let offered = s
        .service
        .remove("massorca.claude", false, false, None)
        .expect("removes");
    let command = claude_uninstall();
    assert_eq!(
        offered.cli[0].action,
        CliAction::Offered {
            command: command.clone()
        }
    );

    let s = installed_claude();
    let ran = s
        .service
        .remove("massorca.claude", false, true, Some(&confirm()))
        .expect("removes");
    assert_eq!(
        ran.cli[0].action,
        CliAction::Ran {
            command: command.clone(),
            exit_code: Some(0)
        }
    );
    assert_eq!(s.host.runs().last(), Some(&command));
    assert!(s.service.layout().uninstall_log("massorca.claude").exists());

    let s = installed(FakeHost::new().with_program("kit"), false);
    let left = s
        .service
        .remove("acme.kit", false, true, None)
        .expect("removes");
    assert_eq!(left.cli[0].action, CliAction::LeftAlone);
    assert!(s.host.runs().is_empty());
}

#[test]
fn a_failed_cli_uninstall_is_reported_after_the_removal() {
    let s = installed_claude_with(FakeHost::new().exit_for("uninstall", 2));
    let removed = s
        .service
        .remove("massorca.claude", false, true, Some(&confirm()))
        .expect("removes");
    assert!(matches!(removed.cli[0].action, CliAction::Failed { .. }));
    assert_eq!(
        s.lock_json().expect("lock")["packages"],
        serde_json::json!([])
    );
}

#[test]
fn a_forged_lockfile_path_is_refused_and_nothing_outside_the_package_is_deleted() {
    let s = setup(FakeHost::new());
    let keep = s.service.layout().data_dir("keep");
    std::fs::create_dir_all(&keep).expect("a folder the forged path would reach");
    std::fs::write(keep.join("notes.txt"), b"kept").expect("a file in it");
    let path = s.service.layout().lock_path();
    std::fs::create_dir_all(path.parent().expect("parent")).expect("the profile folder");
    std::fs::write(
        &path,
        serde_json::json!({ "version": 1, "packages": [{
            "id": "acme.kit", "version": "1.0.0", "source": { "kind": "dir", "ref": "C:/x" },
            "sha256": "ab", "path": "pkgs/acme.kit/../../data/keep", "enabled": true,
            "consent": "c", "installed_at_ms": 0
        }] })
        .to_string(),
    )
    .expect("the forged lockfile");

    let error = s
        .service
        .remove("acme.kit", false, false, None)
        .expect_err("a path outside the package is refused");

    assert_eq!(
        error.code(),
        massorca_core_extensions::codes::INVALID_LOCKFILE
    );
    assert!(
        keep.join("notes.txt").exists(),
        "nothing outside was deleted"
    );
}

#[test]
fn a_changed_installed_manifest_is_never_trusted_or_run() {
    let s = installed_claude();
    let rel = s.lock_json().expect("lock")["packages"][0]["path"]
        .as_str()
        .expect("path")
        .to_owned();
    let path = s.service.layout().resolve(&format!("{rel}/extension.json"));
    std::fs::write(path, b"{\"tampered\": true}").expect("tamper");

    let other = s.package_dir("other", &agent_package("other", "other", "other"));
    s.install(&other, false)
        .expect("a tampered package never blocks installing another");
    let kinds = massorca_core_extensions::installed_agent_kinds(&s.profile)
        .expect("one tampered package never fails the whole call");
    assert_eq!(kinds.len(), 2);
    assert_eq!(kinds[0].kind_id, "claude");
    assert_eq!(kinds[0].path, None);
    assert_eq!(
        kinds[0].error.as_ref().map(|error| error.code.as_str()),
        Some(codes::TAMPERED)
    );
    assert_eq!(kinds[1].kind_id, "other");
    assert!(kinds[1].path.is_some() && kinds[1].error.is_none());
    assert!(s.host.runs().iter().all(|argv| argv[0] != "planted"));
}

#[test]
fn an_uninstall_runs_only_after_its_confirmation_and_nothing_changes_without_it() {
    let s = installed_claude();
    let commands = vec![claude_uninstall()];
    let fingerprint = massorca_core_extensions::review::uninstall_fingerprint(&commands);

    let refused = s.service.remove("massorca.claude", false, true, None);
    let error = refused.expect_err("the uninstall needs its confirmation");
    assert_eq!(error.code(), codes::CONSENT_REQUIRED);
    let data = error.data().expect("the error names what would run");
    assert_eq!(data["uninstall"], serde_json::json!(commands));
    assert_eq!(data["fingerprint"], fingerprint.as_str());
    assert!(
        s.host.runs().iter().all(|argv| argv[1] != "uninstall"),
        "no uninstall ran"
    );
    assert_eq!(
        s.lock_json().expect("lock")["packages"][0]["id"],
        "massorca.claude"
    );

    let wrong = s
        .service
        .remove("massorca.claude", false, true, Some("stale"))
        .expect_err("a different review is refused");
    assert_eq!(wrong.code(), codes::CONSENT_MISMATCH);
    assert_eq!(
        s.lock_json().expect("lock")["packages"][0]["id"],
        "massorca.claude"
    );

    let removed = s
        .service
        .remove("massorca.claude", false, true, Some(&fingerprint))
        .expect("removes with the confirmation");
    assert!(matches!(removed.cli[0].action, CliAction::Ran { .. }));
    assert_eq!(s.host.runs().last(), Some(&claude_uninstall()));
}

#[test]
fn removing_without_an_uninstall_asks_nothing() {
    let s = installed_claude();
    let removed = s
        .service
        .remove("massorca.claude", false, false, None)
        .expect("removes");
    assert!(matches!(removed.cli[0].action, CliAction::Offered { .. }));
}

/// The confirmation a person gives the built-in Claude's uninstall on this host.
fn confirm() -> String {
    massorca_core_extensions::review::uninstall_fingerprint(&[claude_uninstall()])
}
