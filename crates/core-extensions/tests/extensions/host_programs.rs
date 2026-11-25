//! Tool packages (decision 0151): Git, GitHub CLI and Node.js as host programs. Every test runs on the fake host with the
//! Windows commands, so no vendor command or real installer runs.

use massorca_core_extensions::codes;
use massorca_core_extensions::removal::CliAction;
use massorca_core_extensions::source::Source;
use massorca_ext_package::manifest::agent::CliOs;
use serde_json::json;

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
fn the_catalogue_lists_git_github_cli_and_node_as_tools_with_their_detection() {
    let s = windows(FakeHost::new().with_program("git"));

    let items = s.service.catalogue().expect("catalogue");

    for (id, program, detected) in [
        ("massorca.git", "git", true),
        ("massorca.gh", "gh", false),
        ("massorca.node", "node", false),
    ] {
        let item = items.iter().find(|item| item.id == id).expect(id);
        assert_eq!(item.listing.category, "tools", "{id}");
        assert!(item.agents.is_empty(), "{id}");
        assert_eq!(item.programs.len(), 1, "{id}");
        assert_eq!(item.programs[0].id, program, "{id}");
        assert_eq!(item.programs[0].detected, detected, "{id}");
        assert_eq!(item.detected, Some(detected), "{id}");
    }
}

#[test]
fn a_missing_tool_installs_with_the_vendor_command_and_is_verified() {
    let s = windows(FakeHost::new());

    let review = s.service.inspect(&git()).expect("review");
    assert!(review.runs_installer);
    s.install(&git(), true).expect("installs");

    assert_eq!(
        s.host.runs(),
        [
            vec!["winget", "install", "--id", "Git.Git", "-e"],
            vec!["git", "--version"],
        ]
    );
    let lock = s.lock_json().expect("the lockfile");
    assert_eq!(lock["packages"][0]["cli"][0]["agent"], "git");
    assert_eq!(lock["packages"][0]["cli"][0]["installed_by_package"], true);
}

#[test]
fn a_tool_already_on_the_host_is_registered_and_never_uninstalled() {
    let s = windows(FakeHost::new().with_program("git"));
    s.install(&git(), false).expect("registers");
    assert!(s.host.runs().is_empty(), "nothing runs");

    let removed = s
        .service
        .remove("massorca.git", false, true, None)
        .expect("removes the package only");

    assert_eq!(removed.cli[0].action, CliAction::LeftAlone);
    assert!(s.host.runs().is_empty(), "no uninstall ran");
}

#[test]
fn a_tool_massorca_installed_is_uninstalled_only_with_consent() {
    let s = windows(FakeHost::new());
    s.install(&git(), true).expect("installs");

    let error = s
        .service
        .remove("massorca.git", false, true, None)
        .expect_err("the uninstall needs consent");
    let consent = match &error {
        massorca_core_extensions::Error::UninstallConsentRequired {
            commands,
            fingerprint,
        } => {
            assert_eq!(
                commands,
                &[vec!["winget", "uninstall", "--id", "Git.Git", "-e"]]
            );
            fingerprint.clone()
        }
        other => panic!("{other:?}"),
    };
    s.service
        .remove("massorca.git", false, true, Some(&consent))
        .expect("removes");

    assert_eq!(
        s.host.runs().last().expect("a run"),
        &vec!["winget", "uninstall", "--id", "Git.Git", "-e"]
    );
}

#[test]
fn a_third_party_host_program_with_vendor_commands_is_refused() {
    let s = windows(FakeHost::new());
    let manifest = json!({
        "id": "acme.tool", "name": "Tool", "version": "1.0.0", "publisher": "acme",
        "description": "A tool.", "api": "1.0", "requires": { "massorca": ">=0.1.0" },
        "surfaces": ["desktop"], "modes": ["workspaces"],
        "permissions": [{ "kind": "process", "scope": { "programs": ["tool"] }, "reason": "Runs it." }],
        "contributes": { "hostPrograms": [{ "id": "tool", "cli": {
            "binary": "tool", "source": "https://vendor.example/install",
            "install": [{ "os": "windows", "command": ["winget", "install", "Acme.Tool"] }]
        } }] }
    });
    let source = s.package_dir("tool", &manifest);

    let error = s.service.inspect(&source).expect_err("refused");

    assert_eq!(error.code(), codes::VENDOR_COMMAND_REFUSED);
}
