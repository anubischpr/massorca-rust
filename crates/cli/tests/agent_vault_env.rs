//! Vault secrets reach agent runs as environment variables (decision 0071), with the fake agent declared as kind `fake`.
//! The fake agent prints whether each probed variable is set, never its value. Every value here is a placeholder.

mod harness;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Stdio;

use harness::{ChildGuard, Sandbox, eventually, massorca_in, single_document, with_json};
use massorca_client::{Client, ConnectOptions};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::agents::AgentStartParams;
use serde_json::{Value, json};

/// A variable the fake kind strips, which the vault grants.
const STRIPPED: &str = "FAKE_AGENT_STRAY_KEY";
const PROBED: &str = "PROJECT_TOKEN,GLOBAL_TOKEN,FAKE_AGENT_STRAY_KEY,CALLER_WINS";
/// Distinctive placeholders, searched for in every file of the profile afterwards.
const VALUES: [(&str, &str); 4] = [
    ("PROJECT_TOKEN", "placeholder-project-7c21"),
    ("GLOBAL_TOKEN", "placeholder-global-9e04"),
    (STRIPPED, "placeholder-granted-3b77"),
    ("CALLER_WINS", "placeholder-caller-51aa"),
];

fn declare_fake(sandbox: &Sandbox) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    let fake = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"], "strip_env": [STRIPPED] }
    });
    fs::write(dir.join("fake.json"), fake.to_string()).expect("write the fake kind");
}

/// The service at trace level, with the fake agent told which variables to report on.
fn spawn_service(sandbox: &Sandbox) -> ChildGuard {
    sandbox.track("default");
    let child = massorca_in(sandbox.home())
        .args(["--profile", "default", "service", "run"])
        .env("MASSORCA_LOG", "trace")
        .env("FAKE_AGENT_PROBE_ENV", PROBED)
        .env_remove("FAKE_AGENT_HOME")
        .env_remove(STRIPPED)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the service starts");
    let guard = ChildGuard::new(child);
    sandbox.wait_for_running("default", true);
    guard
}

fn set_secret(sandbox: &Sandbox, scope: &str, name: &str, value: &str) {
    sandbox
        .run_with_input(
            "default",
            &["secret", "set", scope, name],
            format!("{value}\n").as_bytes(),
        )
        .success();
}

/// A project with workspace `main` and an imported fake login, so the kind's credential home exists; returns the
/// project id.
fn project_with_login(sandbox: &Sandbox) -> String {
    let notes = sandbox.home().join("notes");
    fs::create_dir_all(&notes).expect("the project folder");
    let added = sandbox.json_ok(
        "default",
        &["project", "add", path_text(&notes), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    let login = sandbox.home().join("fake-login");
    fs::create_dir_all(&login).expect("the login folder");
    fs::write(login.join("token.json"), r#"{"fake":true}"#).expect("the fake token");
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "credentials",
            "import",
            "fake",
            "--from",
            path_text(&login),
        ],
    );
    added["id"].as_str().expect("a project id").to_owned()
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

fn start_fake(sandbox: &Sandbox) -> String {
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "main",
            "--read-only",
        ],
    );
    started["terminal_id"]
        .as_str()
        .expect("a terminal id")
        .to_owned()
}

fn wait_for_line(sandbox: &Sandbox, terminal: &str, line: &str) {
    eventually(&format!("`{line}` on the agent's screen"), || {
        let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
        screen["lines"]
            .as_array()
            .is_some_and(|lines| {
                lines
                    .iter()
                    .any(|shown| shown.as_str().map(str::trim) == Some(line))
            })
            .then_some(())
    });
}

/// Starts the fake agent through the client with `env` from the caller, and returns its terminal id.
fn start_with_caller_env(sandbox: &Sandbox, env: BTreeMap<String, String>) -> String {
    let profile = Profile::resolve_with(
        Some(sandbox.home().to_path_buf()),
        Some("default".to_owned()),
        |_| None,
    )
    .expect("the profile resolves");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    runtime.block_on(async {
        let client = Client::connect(
            &profile,
            ConnectOptions::new(PeerInfo {
                name: "vault-env-test".to_owned(),
                version: "0.0.0-test".to_owned(),
            }),
        )
        .await
        .expect("the client connects");
        let started = client
            .agent_start(&AgentStartParams {
                workspace: "main".to_owned(),
                kind: "fake".to_owned(),
                prompt: None,
                args: Vec::new(),
                env,
                read_only: true,
                title: None,
                cols: None,
                rows: None,
                request_id: None,
            })
            .await
            .expect("the agent starts");
        started.terminal_id.expect("a terminal id")
    })
}

/// Runs `--json agent start fake`, which must fail, and returns the error document.
fn refused_start(sandbox: &Sandbox) -> Value {
    let outcome = sandbox
        .run(
            "default",
            &with_json(&[
                "agent",
                "start",
                "fake",
                "--workspace",
                "main",
                "--read-only",
            ]),
        )
        .failure();
    single_document(&outcome)["error"].clone()
}

fn session_count(sandbox: &Sandbox) -> usize {
    sandbox.json_ok("default", &["agent", "list"])["sessions"]
        .as_array()
        .map_or(0, Vec::len)
}

/// Every file under `dir`, recursively.
fn files_under(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn vault_secrets_reach_the_run_and_never_a_file_of_the_profile() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let service = spawn_service(&sandbox);
    let project = project_with_login(&sandbox);
    let project_scope = format!("project:{project}");
    set_secret(&sandbox, &project_scope, "PROJECT_TOKEN", VALUES[0].1);
    for (name, value) in &VALUES[1..] {
        set_secret(&sandbox, "global", name, value);
    }

    let run = start_fake(&sandbox);
    for name in ["PROJECT_TOKEN", "GLOBAL_TOKEN", STRIPPED, "CALLER_WINS"] {
        wait_for_line(&sandbox, &run, &format!("fake-agent: env {name} set"));
    }

    // The caller's variable wins over the vault's: an empty value is reported as `empty`.
    let caller = start_with_caller_env(
        &sandbox,
        BTreeMap::from([("CALLER_WINS".to_owned(), String::new())]),
    );
    wait_for_line(&sandbox, &caller, "fake-agent: env CALLER_WINS empty");
    wait_for_line(&sandbox, &caller, "fake-agent: env GLOBAL_TOKEN set");

    drop(service);
    sandbox.run("default", &["service", "stop"]).success();
    let mut files = Vec::new();
    files_under(&sandbox.profile_dir("default"), &mut files);
    assert!(
        files
            .iter()
            .any(|path| path.extension().is_some_and(|ext| ext == "log")
                && fs::metadata(path).is_ok_and(|meta| meta.len() > 0)),
        "the trace-level service wrote a log: {files:?}"
    );
    for path in &files {
        let bytes = fs::read(path).unwrap_or_default();
        for (_, value) in VALUES {
            let found = bytes
                .windows(value.len())
                .any(|window| window == value.as_bytes());
            assert!(!found, "{} holds a value", path.display());
        }
    }
}

#[test]
fn a_reserved_name_or_a_locked_vault_fails_the_start_and_leaves_no_session() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let _service = spawn_service(&sandbox);
    project_with_login(&sandbox);

    set_secret(&sandbox, "global", "MASSORCA_X", "placeholder-reserved");
    let reserved = refused_start(&sandbox);
    assert_eq!(reserved["code"], "agent.secrets_unavailable", "{reserved}");
    let message = reserved["message"].as_str().unwrap_or_default();
    assert!(message.contains("MASSORCA_X"), "{message}");
    assert!(!message.contains("placeholder-reserved"), "{message}");
    assert_eq!(session_count(&sandbox), 0, "no session was left behind");
    sandbox
        .run("default", &["secret", "remove", "global", "MASSORCA_X"])
        .success();

    set_secret(&sandbox, "global", "GLOBAL_TOKEN", "placeholder-locked");
    // Without its protection record the vault is locked, and the running service drops its cached key.
    fs::remove_file(
        sandbox
            .profile_dir("default")
            .join("vault")
            .join("protection"),
    )
    .expect("the protection record goes");
    let locked = refused_start(&sandbox);
    assert_eq!(locked["code"], "agent.secrets_unavailable", "{locked}");
    let message = locked["message"].as_str().unwrap_or_default();
    assert!(message.contains("locked"), "{message}");
    assert!(!message.contains("placeholder-locked"), "{message}");
    assert_eq!(session_count(&sandbox), 0, "no session was left behind");
}
