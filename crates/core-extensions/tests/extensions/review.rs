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
            command: vec!["kit-setup".to_owned(), "install".to_owned()]
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
    assert_eq!(value["cli"][0]["command"][0], "kit-setup");
    assert_eq!(value["source"]["kind"], "dir");
}
