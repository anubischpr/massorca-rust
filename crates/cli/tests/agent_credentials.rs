//! `massorca agent credentials ...` with the fake agent declared as kind `fake` (decision 0056).
//!
//! Every credential here is a fake file in a temporary directory; no real agent home is read.

mod harness;

use std::fs;
use std::path::Path;
use std::process::Stdio;

use harness::{
    ChildGuard, Sandbox, eventually, massorca_in, single_document, stdout_text, with_json,
};
use serde_json::{Value, json};

const FAKE_TOKEN: &str = r#"{"fake":true,"refresh":"fake-secret-value"}"#;
/// A variable the fake kind strips; the service is started with it set to a stray value.
const STRIPPED: &str = "FAKE_AGENT_STRAY_KEY";

/// Declares kind `fake` with `files` and strips [`STRIPPED`], and kind `bare`, which declares no files.
fn declare_kinds(sandbox: &Sandbox, files: &[&str]) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    let fake = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "credentials": { "home_env": "FAKE_AGENT_HOME", "files": files, "strip_env": [STRIPPED] }
    });
    fs::write(dir.join("fake.json"), fake.to_string()).expect("write the fake kind");
    let bare = json!({
        "id": "bare",
        "name": "No files",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "credentials": { "home_env": "BARE_HOME" }
    });
    fs::write(dir.join("bare.json"), bare.to_string()).expect("write the bare kind");
}

/// A source directory beside the profile with fake login files, and one file the kind does not declare.
fn fake_source(sandbox: &Sandbox) -> std::path::PathBuf {
    let source = sandbox.home().join("fake-login");
    fs::create_dir_all(&source).expect("create the source");
    fs::write(source.join("token.json"), FAKE_TOKEN).expect("write the fake token");
    fs::write(source.join("config.toml"), "fake = true\n").expect("write the fake config");
    fs::write(source.join("history.jsonl"), "not declared\n").expect("write an extra file");
    source
}

fn home(sandbox: &Sandbox) -> std::path::PathBuf {
    sandbox
        .profile_dir("default")
        .join("agents")
        .join("homes")
        .join("fake")
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// Runs `--json <args>`, which must fail, and returns the error document.
fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

#[test]
fn import_copies_only_the_declared_files_and_prints_names_and_sizes_only() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox, &["token.json", "config.toml"]);
    let source = fake_source(&sandbox);

    let outcome = sandbox
        .run(
            "default",
            &[
                "agent",
                "credentials",
                "import",
                "fake",
                "--from",
                path_text(&source),
            ],
        )
        .success();
    let text = stdout_text(&outcome);
    assert!(text.contains("token.json"), "{text}");
    assert!(text.contains(&FAKE_TOKEN.len().to_string()), "{text}");
    assert!(
        text.contains("single use"),
        "the single-use note is printed: {text}"
    );
    assert!(!text.contains("fake-secret-value"), "no contents: {text}");
    assert!(!text.contains("history.jsonl"), "{text}");

    let mut names: Vec<String> = fs::read_dir(home(&sandbox))
        .expect("the home exists")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["config.toml", "token.json"]);
    assert_eq!(
        fs::read_to_string(home(&sandbox).join("token.json")).unwrap(),
        FAKE_TOKEN
    );

    let logs = sandbox.profile_dir("default").join("logs");
    for entry in fs::read_dir(&logs).into_iter().flatten().flatten() {
        let log = fs::read_to_string(entry.path()).unwrap_or_default();
        assert!(
            !log.contains("fake-secret-value"),
            "a log holds file contents"
        );
    }
}

#[test]
fn a_second_import_needs_replace() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox, &["token.json"]);
    let source = fake_source(&sandbox);
    let import = [
        "agent",
        "credentials",
        "import",
        "fake",
        "--from",
        path_text(&source),
    ];
    sandbox.json_ok("default", &import);

    let error = error_of(&sandbox, &import);
    assert_eq!(error["code"], "agent.credentials_exist");

    fs::write(source.join("token.json"), r#"{"fake":2}"#).unwrap();
    let mut replace = import.to_vec();
    replace.push("--replace");
    let imported = sandbox.json_ok("default", &replace);
    assert_eq!(
        imported["files"],
        json!([{ "name": "token.json", "bytes": 10 }])
    );
    assert_eq!(
        fs::read_to_string(home(&sandbox).join("token.json")).unwrap(),
        r#"{"fake":2}"#
    );
}

#[test]
fn a_missing_declared_file_fails_and_leaves_the_home_unchanged() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox, &["token.json", "config.toml"]);
    let source = fake_source(&sandbox);
    let import = [
        "agent",
        "credentials",
        "import",
        "fake",
        "--from",
        path_text(&source),
        "--replace",
    ];
    sandbox.json_ok("default", &import);

    fs::remove_file(source.join("config.toml")).unwrap();
    fs::write(source.join("token.json"), r#"{"fake":2}"#).unwrap();
    let error = error_of(&sandbox, &import);
    assert_eq!(error["code"], "agent.credentials_invalid");
    assert_eq!(
        error["data"]["files"],
        json!([{ "name": "config.toml", "reason": "missing" }])
    );
    assert_eq!(
        fs::read_to_string(home(&sandbox).join("token.json")).unwrap(),
        FAKE_TOKEN
    );
    assert!(home(&sandbox).join("config.toml").is_file());
}

#[test]
fn a_file_over_one_mebibyte_fails() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox, &["token.json"]);
    let source = fake_source(&sandbox);
    fs::write(source.join("token.json"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
    let error = error_of(
        &sandbox,
        &[
            "agent",
            "credentials",
            "import",
            "fake",
            "--from",
            path_text(&source),
        ],
    );
    assert_eq!(error["code"], "agent.credentials_invalid");
    assert_eq!(error["data"]["files"][0]["reason"], "larger than 1 MiB");
    assert!(!home(&sandbox).exists());
}

/// A link in the source fails the import. Unix always runs it; Windows only when it may make a symbolic link.
#[test]
#[expect(
    clippy::print_stderr,
    reason = "a skipped test must name why it skipped"
)]
fn a_source_symlink_fails_the_import() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox, &["token.json"]);
    let source = sandbox.home().join("linked-login");
    fs::create_dir_all(&source).unwrap();
    let real = sandbox.home().join("real-token.json");
    fs::write(&real, FAKE_TOKEN).unwrap();
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&real, source.join("token.json"));
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(&real, source.join("token.json"));
    if let Err(error) = made {
        eprintln!("skipped: symbolic links cannot be made here ({error})");
        return;
    }
    let error = error_of(
        &sandbox,
        &[
            "agent",
            "credentials",
            "import",
            "fake",
            "--from",
            path_text(&source),
        ],
    );
    assert_eq!(error["code"], "agent.credentials_invalid");
    assert_eq!(error["data"]["files"][0]["reason"], "a link");
    assert!(!home(&sandbox).exists());
}

#[test]
fn a_kind_without_files_is_refused() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox, &["token.json"]);
    let source = fake_source(&sandbox);
    let error = error_of(
        &sandbox,
        &[
            "agent",
            "credentials",
            "import",
            "bare",
            "--from",
            path_text(&source),
        ],
    );
    assert_eq!(error["code"], "agent.credentials_unsupported");
}

#[test]
fn show_before_and_after_an_import_then_remove() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox, &["token.json"]);
    let source = fake_source(&sandbox);

    let before = sandbox.json_ok("default", &["agent", "credentials", "show", "fake"]);
    assert_eq!(before["exists"], false);
    assert_eq!(
        before["files"],
        json!([{ "name": "token.json", "present": false }])
    );

    sandbox.json_ok(
        "default",
        &[
            "agent",
            "credentials",
            "import",
            "fake",
            "--from",
            path_text(&source),
        ],
    );
    let after = sandbox.json_ok("default", &["agent", "credentials", "show", "fake"]);
    assert_eq!(after["exists"], true);
    assert_eq!(after["home"], path_text(&home(&sandbox)));
    assert_eq!(after["files"][0]["present"], true);
    assert_eq!(after["files"][0]["bytes"], FAKE_TOKEN.len());
    assert!(
        after["files"][0]["modified_at_ms"]
            .as_i64()
            .is_some_and(|ms| ms > 0)
    );
    let text = stdout_text(
        &sandbox
            .run("default", &["agent", "credentials", "show", "fake"])
            .success(),
    );
    assert!(!text.contains("fake-secret-value"), "{text}");

    let removed = sandbox.json_ok("default", &["agent", "credentials", "remove", "fake"]);
    assert_eq!(removed, json!({ "kind": "fake", "removed": true }));
    assert!(!home(&sandbox).exists());
    let again = sandbox.json_ok("default", &["agent", "credentials", "remove", "fake"]);
    assert_eq!(again["removed"], false);
}

/// Starts the service with a stray key in its environment and the fake agent told to report on it.
fn spawn_service_with_stray_key(sandbox: &Sandbox) -> ChildGuard {
    sandbox.track("default");
    let child = massorca_in(sandbox.home())
        .args(["--profile", "default", "service", "run"])
        .env(STRIPPED, "stray-value")
        .env("FAKE_AGENT_PROBE_ENV", STRIPPED)
        .env_remove("FAKE_AGENT_HOME")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the service starts");
    let guard = ChildGuard::new(child);
    sandbox.wait_for_running("default", true);
    guard
}

/// Starts a read-only fake agent in workspace `main` and returns its terminal id.
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

/// Waits until the terminal's screen shows `line`.
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

#[test]
fn a_run_uses_the_imported_home_and_empties_the_stripped_keys() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox, &["token.json"]);
    let source = fake_source(&sandbox);
    let _service = spawn_service_with_stray_key(&sandbox);
    let notes = sandbox.home().join("notes");
    fs::create_dir_all(&notes).unwrap();
    sandbox.json_ok(
        "default",
        &["project", "add", path_text(&notes), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);

    let without = start_fake(&sandbox);
    wait_for_line(&sandbox, &without, "fake-agent: credentials missing");
    wait_for_line(
        &sandbox,
        &without,
        &format!("fake-agent: env {STRIPPED} set"),
    );

    sandbox.json_ok(
        "default",
        &[
            "agent",
            "credentials",
            "import",
            "fake",
            "--from",
            path_text(&source),
        ],
    );
    let with = start_fake(&sandbox);
    wait_for_line(&sandbox, &with, "fake-agent: credentials found");
    // The documented limitation: a stripped variable is emptied, not removed.
    wait_for_line(
        &sandbox,
        &with,
        &format!("fake-agent: env {STRIPPED} empty"),
    );
}

#[cfg(unix)]
#[test]
fn the_home_is_0700_and_its_files_0600() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox, &["token.json", "config.toml"]);
    let source = fake_source(&sandbox);
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "credentials",
            "import",
            "fake",
            "--from",
            path_text(&source),
        ],
    );
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&home(&sandbox)), 0o700);
    assert_eq!(mode(&home(&sandbox).join("token.json")), 0o600);
    assert_eq!(mode(&home(&sandbox).join("config.toml")), 0o600);
}

#[test]
fn replace_and_remove_are_refused_while_a_run_of_the_kind_is_live() {
    let sandbox = Sandbox::new();
    declare_kinds(&sandbox, &["token.json"]);
    let source = fake_source(&sandbox);
    let notes = sandbox.home().join("notes");
    fs::create_dir_all(&notes).unwrap();
    sandbox.json_ok(
        "default",
        &["project", "add", path_text(&notes), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    let import = [
        "agent",
        "credentials",
        "import",
        "fake",
        "--from",
        path_text(&source),
        "--replace",
    ];
    sandbox.json_ok("default", &import);

    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "main",
            "--read-only",
            "--prompt",
            "hold",
        ],
    );
    let session = started["id"].as_str().expect("a session id").to_owned();
    let terminal = started["terminal_id"]
        .as_str()
        .expect("a terminal id")
        .to_owned();
    wait_for_line(&sandbox, &terminal, "fake-agent: holding");

    assert_eq!(error_of(&sandbox, &import)["code"], "agent.busy");
    assert_eq!(
        error_of(&sandbox, &["agent", "credentials", "remove", "fake"])["code"],
        "agent.busy"
    );
    assert!(
        home(&sandbox).join("token.json").is_file(),
        "the home is untouched"
    );
    let shown = sandbox.json_ok("default", &["agent", "credentials", "show", "fake"]);
    assert_eq!(shown["exists"], true);

    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);
    sandbox.json_ok("default", &import);
    let removed = sandbox.json_ok("default", &["agent", "credentials", "remove", "fake"]);
    assert_eq!(removed["removed"], true);
}
