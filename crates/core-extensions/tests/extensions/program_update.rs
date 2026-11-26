//! Updating a host program (decision 0151): only one MassOrca installed, only with the reviewed command. Every test runs
//! on the fake host, so no vendor command runs.

use massorca_core_extensions::codes;
use massorca_core_extensions::source::Source;
use massorca_ext_package::manifest::agent::CliOs;

use crate::fakes::FakeHost;
use crate::service::{Setup, setup};

fn windows(host: FakeHost) -> Setup {
    let mut s = setup(host);
    s.service = s.service.with_os(Some(CliOs::Windows));
    s
}

fn git() -> Source {
    Source::Catalogue("massorca.git".to_owned())
}

#[test]
fn a_tool_massorca_installed_updates_with_the_reviewed_vendor_command() {
    let s = windows(FakeHost::new());
    s.install(&git(), true).expect("installs");

    let review = s
        .service
        .review_program_update("massorca.git", "git")
        .expect("review");
    assert_eq!(
        review.command,
        ["winget", "upgrade", "--id", "Git.Git", "-e"]
    );
    let refused = s
        .service
        .update_program("massorca.git", "git", None)
        .expect_err("needs consent");
    assert_eq!(refused.code(), codes::CONSENT_REQUIRED);
    let updated = s
        .service
        .update_program("massorca.git", "git", Some(&review.consent))
        .expect("updates");

    assert_eq!(updated.exit_code, Some(0));
    assert_eq!(
        s.host.runs()[2..],
        [
            vec!["winget", "upgrade", "--id", "Git.Git", "-e"],
            vec!["git", "--version"],
        ]
    );
}

#[test]
fn a_tool_massorca_did_not_install_is_never_updated() {
    let s = windows(FakeHost::new().with_program("git"));
    s.install(&git(), false).expect("registers");

    let error = s
        .service
        .review_program_update("massorca.git", "git")
        .expect_err("not ours");

    assert_eq!(error.code(), codes::PROGRAM_UPDATE_REFUSED);
    assert!(error.to_string().contains("did not install"), "{error}");
    assert!(s.host.runs().is_empty(), "nothing runs");
}

#[test]
fn a_failed_update_says_so_and_keeps_the_package() {
    let s = windows(FakeHost::new().exit_for("upgrade", 1));
    s.install(&git(), true).expect("installs");
    let review = s
        .service
        .review_program_update("massorca.git", "git")
        .expect("review");

    let error = s
        .service
        .update_program("massorca.git", "git", Some(&review.consent))
        .expect_err("the vendor command failed");

    assert_eq!(error.code(), codes::INSTALLER_FAILED);
    let lock = s.lock_json().expect("the package stays");
    assert_eq!(lock["packages"][0]["cli"][0]["installed_by_package"], true);
}
