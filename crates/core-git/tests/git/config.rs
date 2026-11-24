//! Reading one configuration value through the git runner.

use massorca_core_git::Git;

use crate::fixture::{Fixture, run_git};

#[test]
fn config_get_reads_a_set_key_and_reports_a_missing_key_as_none() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["config", "massorca.probe", "set value"]);
    let git = Git::discover().expect("git on PATH");

    assert_eq!(
        git.config_get(&repo, "massorca.probe")
            .expect("read a set key"),
        Some("set value".to_owned())
    );
    assert_eq!(
        git.config_get(&repo, "massorca.missing")
            .expect("read a missing key"),
        None
    );
}
