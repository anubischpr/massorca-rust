//! Files a prompt sends with it through the CLI (decision 0172). A terminal run types a mention for each file and refuses
//! an image; a path that leaves the workspace, a missing file, a link out of it or too many files are refused before
//! anything is typed. A chat run sends its image as an image block. The fake agent is declared as kinds in the test
//! profile, and the workspace is a folder of the sandbox.

mod chat_claude_support;
mod harness;

use std::path::Path;

use chat_claude_support::{await_input, frame, result, sandbox as chat_sandbox, start_chat};
use harness::{Sandbox, eventually, single_document, with_json};
use predicates::str::contains;
use serde_json::{Value, json};

/// Generous upper bound for `--until`; the wait ends as soon as the state is observed.
const UNTIL_SECS: &str = "60";

/// The status table the fake agent reports: start is ready, a prompt works, and a `done` carries the turn's result.
fn status() -> Value {
    json!({
        "mechanism": "env-command",
        "events": {
            "start": "idle", "prompt": "working", "ask": "waiting",
            "stop": "done", "fail": "done:failure"
        },
        "result_field": "result"
    })
}

/// Declares the kind `fake-mention`: the fake agent, typed prompts, and a mention template for a file (decision 0172).
fn declare_mention_kind(sandbox: &Sandbox) {
    let kind = json!({
        "id": "fake-mention",
        "name": "Fake agent that names files",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv", "mention": "@{path}" },
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "resume": ["--resume", "{ref}"],
        "status": status()
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join("fake-mention.json"), kind.to_string()).expect("write a kind file");
}

/// A sandbox with the `fake-mention` kind and a folder workspace `main` whose folder holds `a.txt` and `shot.png`.
fn terminal_sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    declare_mention_kind(&sandbox);
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    std::fs::write(notes.join("a.txt"), "a").expect("a text file");
    std::fs::write(notes.join("shot.png"), b"foobar").expect("an image");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    sandbox
}

/// Starts a `fake-mention` session in `main` and waits until its agent is ready; returns the session id.
fn start(sandbox: &Sandbox) -> String {
    let started = sandbox.json_ok(
        "default",
        &["agent", "start", "fake-mention", "--workspace", "main"],
    );
    let session = started["id"].as_str().expect("a session id").to_owned();
    until(sandbox, &session, "idle");
    session
}

/// `agent status <session> --until <state>`, which must succeed; returns the record it printed.
fn until(sandbox: &Sandbox, session: &str, state: &str) -> Value {
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "status",
            session,
            "--until",
            state,
            "--timeout",
            UNTIL_SECS,
        ],
    )
}

/// Runs `--json <args>`, which must fail, and returns the error code.
fn error_code(sandbox: &Sandbox, args: &[&str]) -> String {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"]["code"]
        .as_str()
        .expect("an error code")
        .to_owned()
}

/// Links the directory `link` to `target`: a junction on Windows, which needs no privilege, and a symlink elsewhere.
/// False when the link cannot be made, which fails the test that asked for it.
fn make_link(target: &Path, link: &Path) -> bool {
    #[cfg(windows)]
    {
        massorca_process::command("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(target)
            .output()
            .is_ok_and(|output| output.status.success())
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
}

#[test]
fn a_file_is_named_in_the_text_a_terminal_run_types() {
    let sandbox = terminal_sandbox();
    let session = start(&sandbox);
    sandbox
        .run(
            "default",
            &["agent", "prompt", &session, "look", "--attach", "a.txt"],
        )
        .success()
        .stdout(contains("handed to the terminal"));
    let done = until(&sandbox, &session, "done");
    assert_eq!(done["result"], "done: look @a.txt", "{done}");
}

#[test]
fn an_image_is_refused_on_a_terminal_run_before_anything_is_typed() {
    let sandbox = terminal_sandbox();
    let session = start(&sandbox);
    assert_eq!(
        error_code(
            &sandbox,
            &["agent", "prompt", &session, "look", "--attach", "shot.png"]
        ),
        "agent.attachment_unsupported"
    );
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(shown["activity"]["state"], "idle", "{shown}");
}

#[test]
fn a_path_that_leaves_the_workspace_is_refused() {
    let sandbox = terminal_sandbox();
    std::fs::write(sandbox.home().join("secret.txt"), "s").expect("a file outside");
    let session = start(&sandbox);
    assert_eq!(
        error_code(
            &sandbox,
            &[
                "agent",
                "prompt",
                &session,
                "look",
                "--attach",
                "../secret.txt"
            ]
        ),
        "agent.attachment_invalid"
    );
}

#[test]
fn a_missing_file_is_refused() {
    let sandbox = terminal_sandbox();
    let session = start(&sandbox);
    assert_eq!(
        error_code(
            &sandbox,
            &["agent", "prompt", &session, "look", "--attach", "nope.txt"]
        ),
        "agent.attachment_invalid"
    );
}

#[test]
fn a_link_that_leaves_the_workspace_is_refused() {
    let sandbox = terminal_sandbox();
    let elsewhere = sandbox.home().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("a folder outside");
    std::fs::write(elsewhere.join("secret.txt"), "s").expect("a file outside");
    let link = sandbox.home().join("notes").join("link");
    assert!(
        make_link(&elsewhere, &link),
        "a junction or symlink can be made"
    );
    assert!(link.join("secret.txt").is_file(), "the link resolves");
    let session = start(&sandbox);
    assert_eq!(
        error_code(
            &sandbox,
            &[
                "agent",
                "prompt",
                &session,
                "look",
                "--attach",
                "link/secret.txt"
            ]
        ),
        "agent.attachment_invalid"
    );
}

#[test]
fn more_than_ten_files_are_refused() {
    let sandbox = terminal_sandbox();
    let session = start(&sandbox);
    let mut args = vec!["agent", "prompt", session.as_str(), "look"];
    for _ in 0..11 {
        args.extend(["--attach", "a.txt"]);
    }
    assert_eq!(error_code(&sandbox, &args), "agent.attachment_invalid");
}

#[test]
fn an_image_is_sent_as_an_image_block_on_a_chat_run() {
    let (sandbox, logs) = chat_sandbox(
        &[
            await_input("user"),
            frame(json!({ "type": "system", "subtype": "init", "claude_code_version": "2.1.330" })),
            result(true),
        ],
        &[],
        &[],
    );
    let notes = sandbox.home().join("notes");
    std::fs::write(notes.join("shot.png"), b"foobar").expect("an image");
    let session = start_chat(&sandbox, "say hello");
    let sent = sandbox.json_ok(
        "default",
        &[
            "agent", "prompt", &session, "look", "--force", "--attach", "shot.png",
        ],
    );
    assert_eq!(sent["receipt"], "sent_to_chat", "{sent}");
    let message = eventually("the image message on the chat's standard input", || {
        logs.stdin_lines().into_iter().find(|line| {
            line["message"]["content"]
                .get(1)
                .is_some_and(|block| block["type"] == "image")
        })
    });
    assert_eq!(
        message["message"]["content"],
        json!([
            { "type": "text", "text": "look" },
            { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "Zm9vYmFy" } }
        ]),
        "{message}"
    );
}
