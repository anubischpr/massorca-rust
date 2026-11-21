//! Phase 2 exit check: a terminal started from the CLI survives every CLI process that touches it.
//!
//! Each `massorca` invocation is a separate process, so every step below is a disconnect and a reconnect. The service
//! that the first step autostarts keeps the shell running between them, and the guard at the end stops the service.

mod harness;

use harness::{Sandbox, eventually, stdout_text};

const MARKER: &str = "phase2-exit-marker";

#[test]
fn a_terminal_started_from_the_cli_keeps_running_and_its_output_is_read_by_a_later_process() {
    let sandbox = Sandbox::new();
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);

    // 1. Starts the terminal; this process autostarts the service and exits.
    let created = sandbox.json_ok("default", &["terminal", "create", "main"]);
    let id = created["id"].as_str().expect("terminal id").to_owned();

    // 2. Types into it from a new process.
    sandbox
        .run("default", &["terminal", "send", &id, "echo", MARKER])
        .success();

    // 3. Shows it from another new process: still live.
    let shown = sandbox.json_ok("default", &["terminal", "show", &id]);
    assert_eq!(
        shown["state"], "live",
        "the terminal outlived its creator: {shown}"
    );

    // 4. Reads its screen from yet another process, until the marker is printed on its own line.
    eventually("the marker on the screen", || {
        let text = stdout_text(
            &sandbox
                .run("default", &["terminal", "screen", &id])
                .success(),
        );
        text.lines().any(|line| line.trim() == MARKER).then_some(())
    });

    // 5. The service that carried all of it is still running; the sandbox's guard stops it when the test ends.
    assert_eq!(sandbox.status("default")["running"], true);
}
