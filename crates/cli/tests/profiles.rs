//! Profiles: one service and one data set per profile, and how the profile and the home are chosen.

mod harness;

use harness::{Sandbox, execute, massorca_in, single_document};

#[test]
fn two_profiles_in_one_home_run_isolated_services() {
    let sandbox = Sandbox::new();

    let alpha = sandbox.json_ok("alpha", &["service", "info"]);
    assert_eq!(sandbox.status("beta")["running"], false);
    let beta = sandbox.json_ok("beta", &["service", "info"]);

    assert_eq!(alpha["profile"], "alpha");
    assert_eq!(beta["profile"], "beta");
    assert_ne!(alpha["pid"], beta["pid"]);
    assert!(sandbox.profile_dir("alpha").is_dir());
    assert!(sandbox.profile_dir("beta").is_dir());

    sandbox.json_ok("alpha", &["service", "stop"]);
    assert_eq!(sandbox.status("alpha")["running"], false);
    let beta_status = sandbox.status("beta");
    assert_eq!(beta_status["running"], true);
    assert_eq!(beta_status["info"]["pid"], beta["pid"]);
}

#[test]
fn home_flag_takes_precedence_over_the_home_environment_variable() {
    let sandbox = Sandbox::new();
    let decoy = tempfile::tempdir().expect("a second temporary home");
    sandbox.track("default");

    execute(
        massorca_in(decoy.path())
            .arg("--home")
            .arg(sandbox.home())
            .args(["service", "info"]),
    )
    .success();

    assert!(sandbox.profile_dir("default").is_dir());
    assert!(!decoy.path().join("profiles").exists());
}

#[test]
fn profile_environment_variable_selects_the_profile_when_no_flag_is_given() {
    let sandbox = Sandbox::new();
    sandbox.track("gamma");

    let outcome = execute(
        massorca_in(sandbox.home())
            .env("MASSORCA_PROFILE", "gamma")
            .args(["--json", "service", "info"]),
    )
    .success();

    assert_eq!(single_document(&outcome)["result"]["profile"], "gamma");
    assert!(sandbox.profile_dir("gamma").is_dir());
}
