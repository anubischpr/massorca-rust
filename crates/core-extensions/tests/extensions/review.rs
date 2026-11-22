//! The install review: the CLI plan for this host and the consent fingerprint.

use massorca_core_extensions::host::current_os;
use massorca_core_extensions::review::{PlanState, review};
use massorca_core_extensions::source::{Source, load};

use crate::fakes::FakeHost;
use crate::fixture::{agent_package, package_files, write_dir};

fn kit_review(host: &FakeHost) -> massorca_core_extensions::review::Review {
    let dir = tempfile::tempdir().expect("temp dir");
    write_dir(
        dir.path(),
        &package_files(&agent_package("acme", "kit", "kit")),
    );
    let package = load(&Source::Dir(dir.path().to_path_buf()), None).expect("loads");
    review(&package, host, current_os())
}

#[test]
fn a_missing_cli_plans_the_vendor_command_for_this_os() {
    let review = kit_review(&FakeHost::new());
    assert_eq!(review.id, "acme.kit");
    assert_eq!(review.risk.level, "high");
    assert_eq!(review.listing.category, "agents");
    assert!(review.runs_installer);
    assert_eq!(
        review.cli[0].state,
        PlanState::WillRun {
            command: vec![
                "npm".to_owned(),
                "install".to_owned(),
                "-g".to_owned(),
                "kit".to_owned()
            ]
        }
    );
    assert_eq!(
        review.consent,
        kit_review(&FakeHost::new()).consent,
        "the fingerprint is stable"
    );
}

#[test]
fn a_detected_cli_plans_nothing_to_run_and_changes_the_fingerprint() {
    let detected = kit_review(&FakeHost::new().with_program("kit"));
    assert!(!detected.runs_installer);
    assert!(matches!(detected.cli[0].state, PlanState::Detected { .. }));
    assert_ne!(detected.consent, kit_review(&FakeHost::new()).consent);
}

#[test]
fn a_review_serializes_the_plan_state_inline() {
    let review = kit_review(&FakeHost::new());
    let value = serde_json::to_value(&review).expect("serializes");
    assert_eq!(value["cli"][0]["state"], "will_run");
    assert_eq!(value["cli"][0]["command"][0], "npm");
    assert_eq!(value["source"]["kind"], "dir");
}

/// The fingerprint of a manifest with the package hash held fixed, so only what the fingerprint itself names counts.
fn fingerprint_of(manifest: &serde_json::Value) -> String {
    let manifest = massorca_ext_package::load(&serde_json::to_vec(manifest).expect("json"))
        .expect("a valid manifest");
    let plan = massorca_core_extensions::review::plan(&manifest, &FakeHost::new(), current_os());
    massorca_core_extensions::review::fingerprint(&manifest, "ab", &plan, current_os())
}

#[test]
fn the_fingerprint_covers_the_verify_and_uninstall_commands() {
    let base = agent_package("acme", "kit", "kit");
    let cli = "/contributes/agentAdapters/0/cli";
    let mut other_verify = base.clone();
    *other_verify
        .pointer_mut(&format!("{cli}/verify"))
        .expect("verify") = serde_json::json!(["kit", "--help"]);
    let mut other_uninstall = base.clone();
    for step in other_uninstall
        .pointer_mut(&format!("{cli}/uninstall"))
        .and_then(serde_json::Value::as_array_mut)
        .expect("steps")
    {
        step["command"] = serde_json::json!(["npm", "uninstall", "-g", "kit", "--everything"]);
    }

    let consent = fingerprint_of(&base);
    assert_ne!(consent, fingerprint_of(&other_verify), "verify");
    assert_ne!(consent, fingerprint_of(&other_uninstall), "uninstall");
}

#[test]
fn the_review_shows_what_each_agent_kind_starts_and_reads_and_each_mcp_command() {
    let mut files = package_files(&agent_package("acme", "kit", "kit"));
    files.insert(
        "agents/kit.json".to_owned(),
        br#"{"id":"kit","program":["kit"],"args":["--fast"],"env":{"KIT_LOG":"1"},
            "credentials":{"home_env":"KIT_HOME","files":["auth.json"],"strip_env":["KIT_TOKEN"]}}"#
            .to_vec(),
    );
    let dir = tempfile::tempdir().expect("temp dir");
    write_dir(dir.path(), &files);
    let package = load(&Source::Dir(dir.path().to_path_buf()), None).expect("loads");

    let review = review(&package, &FakeHost::new(), current_os());

    let kind = &review.kinds[0];
    assert!(kind.readable);
    assert_eq!(kind.program, ["kit"]);
    assert_eq!(kind.args, ["--fast"]);
    assert_eq!(kind.env, ["KIT_HOME", "KIT_LOG", "KIT_TOKEN"]);
    assert_eq!(kind.credential_home_env.as_deref(), Some("KIT_HOME"));
    assert_eq!(kind.credential_files, ["auth.json"]);
    assert!(review.mcp_servers.is_empty());
}
