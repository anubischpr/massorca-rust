//! The run log of a package: its install, update, rollback and remove runs, newest last, bounded, and kept after removal.
//! A change refused before it touches anything is not a run.

use std::fs;

use massorca_core_extensions::codes;
use massorca_core_extensions::run_log::{RunAction, RunOutcome};
use massorca_core_extensions::source::Source;
use serde_json::{Value, json};

use crate::fakes::FakeHost;
use crate::fixture::agent_package;
use crate::service::{Setup, setup};

fn kit_manifest(version: &str) -> Value {
    let mut manifest = agent_package("acme", "kit", "kit");
    manifest["version"] = json!(version);
    manifest
}

fn consent_of(s: &Setup, version: &str) -> String {
    let source = s.package_dir("kit", &kit_manifest(version));
    s.service.inspect(&source).expect("inspect").consent
}

fn update_to(s: &Setup, version: &str) {
    let consent = consent_of(s, version);
    s.service
        .update("acme.kit", Some(&consent), None)
        .expect("updates");
}

#[test]
fn install_update_rollback_and_remove_are_each_one_run_newest_last_and_outlive_the_package() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &kit_manifest("1.2.0"));
    s.install(&source, false).expect("installs");
    update_to(&s, "1.3.0");
    let consent = consent_of(&s, "1.2.0");
    s.service
        .rollback("acme.kit", Some(&consent))
        .expect("rolls back");
    s.service
        .remove("acme.kit", false, false, None)
        .expect("removes");

    let runs = s.service.runs("acme.kit").expect("the log");
    let actions: Vec<RunAction> = runs.iter().map(|run| run.action).collect();
    assert_eq!(
        actions,
        [
            RunAction::Install,
            RunAction::Update,
            RunAction::Rollback,
            RunAction::Remove
        ]
    );
    assert_eq!(runs[0].from_version, None);
    assert_eq!(runs[0].to_version.as_deref(), Some("1.2.0"));
    assert_eq!(runs[1].from_version.as_deref(), Some("1.2.0"));
    assert_eq!(runs[1].to_version.as_deref(), Some("1.3.0"));
    assert_eq!(runs[2].from_version.as_deref(), Some("1.3.0"));
    assert_eq!(runs[2].to_version.as_deref(), Some("1.2.0"));
    assert_eq!(runs[3].from_version.as_deref(), Some("1.2.0"));
    assert_eq!(runs[3].to_version, None);
    assert!(runs.iter().all(|run| run.outcome == RunOutcome::Ok));
    assert!(
        runs.iter().all(|run| run.output.is_empty()),
        "no vendor command ran"
    );
    assert!(runs.windows(2).all(|pair| pair[0].at_ms <= pair[1].at_ms));
}

#[test]
fn a_refused_change_is_not_a_run() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &kit_manifest("1.2.0"));

    let no_consent = s
        .service
        .install(&source, None, false, None)
        .expect_err("no consent");
    assert_eq!(no_consent.code(), codes::CONSENT_REQUIRED);
    let no_update = s
        .service
        .update("acme.kit", None, None)
        .expect_err("not installed");
    assert_eq!(no_update.code(), codes::NOT_INSTALLED);

    assert!(s.service.runs("acme.kit").expect("the log").is_empty());
}

#[test]
fn the_log_keeps_the_newest_twenty_runs() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &kit_manifest("1.2.0"));
    s.install(&source, false).expect("installs");
    update_to(&s, "1.3.0");
    let mut last = "1.3.0";
    for _ in 0..21 {
        last = if last == "1.2.0" { "1.3.0" } else { "1.2.0" };
        let consent = consent_of(&s, last);
        s.service
            .rollback("acme.kit", Some(&consent))
            .expect("rolls");
    }

    let runs = s.service.runs("acme.kit").expect("the log");
    assert_eq!(runs.len(), 20);
    assert!(runs.iter().all(|run| run.action == RunAction::Rollback));
    assert_eq!(
        runs.last().and_then(|run| run.to_version.as_deref()),
        Some(last)
    );
}

#[test]
fn an_installer_run_keeps_the_tail_of_its_own_output() {
    let s = setup(FakeHost::new());
    let source = Source::Catalogue("massorca.codex".to_owned());
    s.install(&source, true).expect("installs");

    let runs = s.service.runs("massorca.codex").expect("the log");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].action, RunAction::Install);
    assert_eq!(runs[0].outcome, RunOutcome::Ok);
    assert!(
        runs[0]
            .output
            .iter()
            .any(|line| line.contains("npm install -g @openai/codex -> 0")),
        "{:?}",
        runs[0].output
    );
}

#[test]
fn a_failed_installer_run_is_recorded_as_failed_with_its_output() {
    let s = setup(FakeHost::new().exit_for("install", 3));
    let source = Source::Catalogue("massorca.codex".to_owned());
    s.install(&source, true).expect_err("the installer fails");

    let runs = s.service.runs("massorca.codex").expect("the log");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].outcome, RunOutcome::Failed);
    assert!(runs[0].output.iter().any(|line| line.contains("-> 3")));
}

#[test]
fn an_unknown_id_has_no_runs_and_an_id_that_is_not_one_is_refused() {
    let s = setup(FakeHost::new());

    assert!(s.service.runs("acme.kit").expect("the log").is_empty());
    let error = s.service.runs("../acme.kit").expect_err("not an id");
    assert_eq!(error.code(), codes::NOT_FOUND);
}

#[test]
fn a_log_file_that_cannot_be_read_is_empty_and_the_next_run_starts_it_again() {
    let s = setup(FakeHost::new());
    let path = s.service.layout().run_log("acme.kit");
    fs::create_dir_all(path.parent().expect("a folder")).expect("the logs folder");
    fs::write(&path, "not json").expect("a broken log");
    assert!(s.service.runs("acme.kit").expect("the log").is_empty());

    let source = s.package_dir("kit", &kit_manifest("1.2.0"));
    s.install(&source, false).expect("installs");

    assert_eq!(s.service.runs("acme.kit").expect("the log").len(), 1);
}
