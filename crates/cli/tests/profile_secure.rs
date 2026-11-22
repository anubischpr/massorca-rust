//! A home other programs could change: an autostarted service's refusal reaches the user, and `profile secure` is the
//! explicit way to make the home private (ADR 0073). Temporary homes only.

mod harness;

use std::path::Path;

use harness::{Sandbox, single_document};
use predicates::str::contains;

/// Gives everyone write access to the home, as an older build left a home that inherited a shared folder's access.
fn share(home: &Path) {
    #[cfg(windows)]
    {
        let granted = std::process::Command::new("icacls")
            .arg(home)
            .args(["/grant", "*S-1-1-0:(OI)(CI)(M)"])
            .output()
            .expect("icacls runs");
        assert!(
            granted.status.success(),
            "{}",
            String::from_utf8_lossy(&granted.stderr)
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o777)).expect("chmod");
    }
}

#[test]
fn an_autostarted_service_that_refuses_the_home_reports_its_code_not_a_timeout() {
    let sandbox = Sandbox::new();
    share(sandbox.home());

    let refused = sandbox
        .run("default", &["--json", "service", "info"])
        .code(1);

    let document = single_document(&refused);
    assert_eq!(document["ok"], false, "{document}");
    assert_eq!(
        document["error"]["code"], "profile.unsafe_location",
        "{document}"
    );
    let message = document["error"]["message"].as_str().expect("a message");
    assert!(
        message.contains("other programs on this computer could change these files"),
        "{message}"
    );
    assert!(message.contains("massorca profile secure"), "{message}");

    sandbox
        .run("default", &["service", "info"])
        .code(1)
        .stderr(contains("(profile.unsafe_location)"))
        .stderr(contains("massorca profile secure"));
}

#[test]
fn profile_secure_prints_its_plan_and_changes_the_home_only_with_yes() {
    let sandbox = Sandbox::new();
    share(sandbox.home());

    let plan = single_document(
        &sandbox
            .run("default", &["--json", "profile", "secure"])
            .code(0),
    );
    assert_eq!(plan["result"]["applied"], false, "{plan}");
    let home = &plan["result"]["folders"][0];
    assert_eq!(home["path"], sandbox.home().display().to_string(), "{plan}");
    let removed = home["remove"].as_array().expect("entries to remove");
    #[cfg(windows)]
    assert!(
        removed
            .iter()
            .any(|entry| entry.as_str().is_some_and(|text| text.contains(";WD)"))),
        "{plan}"
    );
    #[cfg(unix)]
    assert_eq!(removed, &vec![serde_json::json!("mode 0777")], "{plan}");
    sandbox
        .run("default", &["profile", "secure"])
        .code(0)
        .stdout(contains("remove"))
        .stdout(contains("massorca profile secure --yes"));
    sandbox.run("default", &["service", "info"]).code(1);

    let applied = single_document(
        &sandbox
            .run("default", &["--json", "profile", "secure", "--yes"])
            .code(0),
    );
    assert_eq!(applied["result"]["applied"], true, "{applied}");
    sandbox.run("default", &["service", "info"]).code(0);
    let again = single_document(
        &sandbox
            .run("default", &["--json", "profile", "secure"])
            .code(0),
    );
    let unchanged = again["result"]["folders"]
        .as_array()
        .expect("folders")
        .iter()
        .all(|folder| folder["remove"].as_array().is_some_and(Vec::is_empty));
    assert!(unchanged, "{again}");
}
