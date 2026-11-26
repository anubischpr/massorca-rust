//! `massorca agent title` (decision 0167): names a chat from its first prompt and reply by one run of the kind's
//! non-interactive form. Every credential is a fake file in a temporary folder. The fake agent answers the run with the
//! reply its `--fake-mode` argument names, and writes its working folder and run identity variables to the file that
//! `FAKE_AGENT_ONESHOT_REPORT` names.

mod harness;

use std::fs;
use std::path::Path;

use harness::{Sandbox, single_document, stdout_text, with_json};
use serde_json::{Value, json};

/// A fake token: the fake agent's check finds it signed in.
const FAKE_TOKEN: &str = r#"{"fake":true,"refresh":"fake-secret-value"}"#;

/// Declares kind `fake` with its sign-in and check, and its one-shot form answering with `mode` (`None` declares no
/// one-shot form).
fn declare_kind(sandbox: &Sandbox, mode: Option<&str>) {
    declare_kind_with(sandbox, mode, &[]);
}

/// [`declare_kind`], with the optional arguments of its one-shot form (each passed only when the fake's `--help` lists it).
fn declare_kind_with(sandbox: &Sandbox, mode: Option<&str>, optional_args: &[Value]) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    let mut kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] },
        "accounts": {
            "login": ["login"],
            "logout": ["logout"],
            "check": {
                "args": ["whoami"],
                "signed_in_exit": 0,
                "expired_pattern": "expired",
                "identity_pattern": "signed in as (\\S+)"
            }
        }
    });
    if let Some(mode) = mode {
        kind["oneshot"] = json!({
            "args": ["-p", "{prompt}", "--output-format", "text", "--fake-mode", mode]
        });
        if !optional_args.is_empty() {
            kind["oneshot"]["optional_args"] = Value::Array(optional_args.to_vec());
        }
    }
    fs::write(dir.join("fake.json"), kind.to_string()).expect("write the fake kind");
}

/// Imports one fake account of kind `fake` from a fake token and makes it the kind's active account.
fn signed_in_account(sandbox: &Sandbox) {
    let source = sandbox.home().join("token-source");
    fs::create_dir_all(&source).expect("create the token folder");
    fs::write(source.join("token.json"), FAKE_TOKEN).expect("write the fake token");
    let source = source.to_str().expect("a UTF-8 path");
    let imported = sandbox.json_ok(
        "default",
        &[
            "agent", "accounts", "import", "fake", "--from", source, "--label", "work",
        ],
    );
    let id = imported["account"]["id"]
        .as_str()
        .expect("the import names the account")
        .to_owned();
    sandbox.json_ok("default", &["agent", "accounts", "activate", "fake", &id]);
}

/// `agent title` for kind `fake` and the first prompt `prompt`.
fn title_args(prompt: &str) -> Vec<&str> {
    vec!["agent", "title", "--kind", "fake", "--prompt", prompt]
}

/// Runs `--json` `args`, which must fail, and returns the error object.
fn title_error(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

#[test]
fn a_chat_is_named_by_the_reply_of_the_one_shot_form_as_a_slug() {
    let sandbox = Sandbox::new();
    declare_kind(&sandbox, Some("say:Explora el proyecto"));
    signed_in_account(&sandbox);

    let named = sandbox.json_ok("default", &title_args("Explora el proyecto"));
    assert_eq!(named["slug"], "explora-el-proyecto", "{named}");

    let text = sandbox
        .run(
            "default",
            &[
                "agent",
                "title",
                "--kind",
                "fake",
                "--prompt",
                "Explora el proyecto",
            ],
        )
        .success();
    assert_eq!(stdout_text(&text).trim(), "explora-el-proyecto");
}

#[test]
fn the_run_has_no_run_identity_and_works_in_a_folder_that_is_gone_afterwards() {
    let sandbox = Sandbox::new();
    let report = sandbox.home().join("oneshot-report.json");
    sandbox.set_env(
        "FAKE_AGENT_ONESHOT_REPORT",
        report.to_str().expect("a UTF-8 path"),
    );
    sandbox.set_env("MASSORCA_AGENT_SESSION", "probe-session");
    sandbox.set_env("MASSORCA_AGENT_TOKEN", "probe-token");
    sandbox.set_env("MASSORCA_AGENT_KIND", "fake");
    declare_kind(&sandbox, Some("say:Fix login"));
    signed_in_account(&sandbox);

    let named = sandbox.json_ok("default", &title_args("Fix the login bug"));
    assert_eq!(named["slug"], "fix-login", "{named}");

    let run: Value = serde_json::from_str(
        &fs::read_to_string(&report).expect("the one-shot run wrote its report"),
    )
    .expect("the report is JSON");
    assert_eq!(
        run["run_identity"],
        json!({
            "MASSORCA_AGENT_SESSION": "unset",
            "MASSORCA_AGENT_TOKEN": "unset",
            "MASSORCA_AGENT_KIND": "unset"
        }),
        "{run}"
    );
    let folder = Path::new(run["cwd"].as_str().expect("the report names the folder"));
    assert!(
        folder.starts_with(sandbox.profile_dir("default")),
        "the folder is in the profile, outside every workspace: {}",
        folder.display()
    );
    assert!(!folder.exists(), "the folder is removed after the run");
}

#[test]
fn a_run_that_exits_with_a_failure_is_oneshot_failed() {
    let sandbox = Sandbox::new();
    declare_kind(&sandbox, Some("exit:3"));
    signed_in_account(&sandbox);
    let error = title_error(&sandbox, &title_args("Fix the login bug"));
    assert_eq!(error["code"], "agent.oneshot_failed", "{error}");
}

#[test]
fn a_run_that_prints_no_title_is_oneshot_failed() {
    let sandbox = Sandbox::new();
    declare_kind(&sandbox, Some("empty"));
    signed_in_account(&sandbox);
    let error = title_error(&sandbox, &title_args("Fix the login bug"));
    assert_eq!(error["code"], "agent.oneshot_failed", "{error}");
}

#[test]
fn a_run_past_its_time_limit_is_oneshot_timeout() {
    let sandbox = Sandbox::new();
    sandbox.set_env("MASSORCA_TITLE_TIMEOUT_MS", "500");
    declare_kind(&sandbox, Some("hang"));
    signed_in_account(&sandbox);
    let error = title_error(&sandbox, &title_args("Fix the login bug"));
    assert_eq!(error["code"], "agent.oneshot_timeout", "{error}");
}

#[test]
fn a_kind_without_a_one_shot_form_cannot_name_a_chat() {
    let sandbox = Sandbox::new();
    declare_kind(&sandbox, None);
    signed_in_account(&sandbox);
    let error = title_error(&sandbox, &title_args("Fix the login bug"));
    assert_eq!(error["code"], "agent.oneshot_unsupported", "{error}");
}

#[test]
fn a_kind_with_no_account_to_run_with_is_refused() {
    let sandbox = Sandbox::new();
    declare_kind(&sandbox, Some("say:Fix login"));
    let error = title_error(&sandbox, &title_args("Fix the login bug"));
    assert_eq!(error["code"], "agent.credentials_invalid", "{error}");
}

#[test]
fn only_the_optional_arguments_the_help_lists_are_passed() {
    let sandbox = Sandbox::new();
    let report = sandbox.home().join("oneshot-report.json");
    sandbox.set_env(
        "FAKE_AGENT_ONESHOT_REPORT",
        report.to_str().expect("a UTF-8 path"),
    );
    sandbox.set_env("FAKE_AGENT_HELP_FLAGS", "--max-turns,--strict-mcp-config");
    declare_kind_with(
        &sandbox,
        Some("say:Fix login"),
        &[
            json!({ "flag": "--max-turns", "values": ["1"] }),
            json!({ "flag": "--strict-mcp-config" }),
            json!({ "flag": "--no-session-persistence" }),
            json!({ "flag": "--tools", "values": [""] }),
        ],
    );
    signed_in_account(&sandbox);

    let named = sandbox.json_ok("default", &title_args("Fix the login bug"));
    assert_eq!(named["slug"], "fix-login", "{named}");

    let run: Value = serde_json::from_str(
        &fs::read_to_string(&report).expect("the one-shot run wrote its report"),
    )
    .expect("the report is JSON");
    let argv: Vec<&str> = run["argv"]
        .as_array()
        .expect("the report names the arguments")
        .iter()
        .map(|arg| arg.as_str().expect("an argument is a string"))
        .collect();
    assert!(
        argv.windows(2)
            .any(|pair| pair[0] == "--max-turns" && pair[1] == "1"),
        "a listed flag is passed with its value: {argv:?}"
    );
    assert!(
        argv.contains(&"--strict-mcp-config"),
        "a listed flag is passed: {argv:?}"
    );
    assert!(
        !argv.contains(&"--no-session-persistence"),
        "an unlisted flag is not passed: {argv:?}"
    );
    assert!(
        !argv.contains(&"--tools"),
        "an unlisted flag is not passed: {argv:?}"
    );
}
