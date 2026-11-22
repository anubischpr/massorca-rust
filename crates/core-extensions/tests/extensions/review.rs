//! The install review: the CLI plan for this host, the consent fingerprint, and what each agent kind declares.

use massorca_core_extensions::host::current_os;
use massorca_core_extensions::review::{PlanState, review};
use massorca_core_extensions::source::{Source, load};

use crate::fakes::FakeHost;
use crate::fixture::{agent_package, package_files, write_dir};

/// The built-in Claude package's install command for this host, as its declaration names it.
fn claude_install() -> Vec<String> {
    let argv: &[&str] = match current_os() {
        Some(massorca_ext_package::manifest::agent::CliOs::Windows) => {
            &["winget", "install", "Anthropic.ClaudeCode"]
        }
        Some(massorca_ext_package::manifest::agent::CliOs::Macos) => {
            &["brew", "install", "--cask", "claude-code"]
        }
        _ => &["npm", "install", "-g", "@anthropic-ai/claude-code@latest"],
    };
    argv.iter().map(|part| (*part).to_owned()).collect()
}

fn claude_review(host: &FakeHost) -> massorca_core_extensions::review::Review {
    let source = Source::Catalogue("massorca.claude".to_owned());
    let package = load(&source, None).expect("loads");
    review(&package, host, current_os())
}

#[test]
fn a_missing_cli_plans_the_built_in_install_command_for_this_os() {
    let review = claude_review(&FakeHost::new());
    assert_eq!(review.id, "massorca.claude");
    assert_eq!(review.listing.category, "agents");
    assert!(review.runs_installer);
    assert_eq!(
        review.cli[0].state,
        PlanState::WillRun {
            command: claude_install()
        }
    );
}

#[test]
fn a_detected_cli_plans_nothing_to_run_and_changes_the_fingerprint() {
    let missing = claude_review(&FakeHost::new());
    let detected = claude_review(&FakeHost::new().with_program("claude"));
    assert!(!detected.runs_installer);
    assert!(matches!(detected.cli[0].state, PlanState::Detected { .. }));
    assert_ne!(missing.consent, detected.consent);
}

#[test]
fn a_review_serializes_the_plan_state_inline() {
    let review = claude_review(&FakeHost::new());
    let value = serde_json::to_value(&review).expect("the review serializes");
    assert_eq!(value["cli"][0]["state"], "will_run");
    assert_eq!(value["cli"][0]["command"][0], claude_install()[0]);
}

#[test]
fn the_fingerprint_is_the_same_for_the_same_review_and_differs_between_packages() {
    let first = claude_review(&FakeHost::new());
    let again = claude_review(&FakeHost::new());
    let codex = {
        let source = Source::Catalogue("massorca.codex".to_owned());
        let package = load(&source, None).expect("loads");
        review(&package, &FakeHost::new(), current_os())
    };
    assert_eq!(first.consent, again.consent);
    assert_ne!(first.consent, codex.consent);
}

#[test]
fn the_review_shows_what_each_agent_kind_starts_and_reads_and_each_mcp_command() {
    let mut files = package_files(&agent_package("acme", "kit", "kit"));
    files.insert(
        "agents/kit.json".to_owned(),
        br#"{"id":"kit","program":["kit"],"args":["--fast"],
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
    assert_eq!(kind.env, ["KIT_TOKEN"]);
    assert_eq!(kind.credential_home_env.as_deref(), Some("KIT_HOME"));
    assert_eq!(kind.credential_files, ["auth.json"]);
    assert!(review.mcp_servers.is_empty());
}
