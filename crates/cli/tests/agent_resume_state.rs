//! A session's resume state (decision 0160): whether `agent resume` could start a new run, and why not. The saved
//! conversations here are empty files in a fake user home; the fake agent plays a CLI that has lost one.

mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use harness::{
    ChildGuard, Sandbox, eventually, massorca_in, single_document, stdout_text, with_json,
};
use serde_json::{Value, json};

const NOT_FOUND_REASON: &str = "the agent saved no conversation for this session";

/// Declares `kind` as the fake agent with a resume form and `extra` fields.
fn declare(sandbox: &Sandbox, kind: &str, extra: Value) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    let mut declared = json!({
        "id": kind,
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "resume": ["--resume", "{ref}"],
        "status": { "mechanism": "env-command", "events": { "start": "idle", "stop": "done" } }
    });
    for (key, value) in extra.as_object().expect("an object") {
        declared[key] = value.clone();
    }
    fs::write(dir.join(format!("{kind}.json")), declared.to_string()).expect("write a kind");
}

/// A fake user home; Windows resolves the service's own known folders below `USERPROFILE`.
fn fake_user_home(sandbox: &Sandbox) -> PathBuf {
    let home = sandbox.home().join("user");
    for known in [["AppData", "Roaming"], ["AppData", "Local"]] {
        fs::create_dir_all(home.join(known[0]).join(known[1])).expect("create a known folder");
    }
    home
}

/// Starts the service with its home at `user_home`, no vendor variable of the developer's, and `env`.
fn spawn_service(sandbox: &Sandbox, user_home: &Path, env: &[(&str, &str)]) -> ChildGuard {
    sandbox.track("default");
    let mut command = massorca_in(sandbox.home());
    command
        .args(["--profile", "default", "service", "run"])
        .env("HOME", user_home)
        .env("USERPROFILE", user_home)
        .env_remove("FAKE_AGENT_HOME")
        .env_remove("FAKE_TRANSCRIPT_DIR")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for name in ["CLAUDE_CONFIG_DIR", "CODEX_HOME", "PI_CODING_AGENT_DIR"] {
        command.env_remove(name);
    }
    for (name, value) in env {
        command.env(name, value);
    }
    let guard = ChildGuard::new(command.spawn().expect("the service starts"));
    sandbox.wait_for_running("default", true);
    guard
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// A project folder with workspace `main`.
fn add_workspace(sandbox: &Sandbox) {
    let notes = sandbox.home().join("notes");
    fs::create_dir_all(&notes).unwrap();
    sandbox.json_ok(
        "default",
        &["project", "add", path_text(&notes), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
}

/// Starts `kind` in `main`, waits for its first report, stops it, and returns the exited session.
fn run_and_stop(sandbox: &Sandbox, kind: &str) -> Value {
    let started = sandbox.json_ok("default", &["agent", "start", kind, "--workspace", "main"]);
    let session = started["id"].as_str().expect("a session id").to_owned();
    eventually("the first report", || {
        let shown = sandbox.json_ok("default", &["agent", "status", &session]);
        (shown["activity"]["state"] == "idle").then_some(())
    });
    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);
    exited(sandbox, &session)
}

/// The session once its run has exited.
fn exited(sandbox: &Sandbox, session: &str) -> Value {
    eventually("the run to exit", || {
        let shown = sandbox.json_ok("default", &["agent", "status", session]);
        (shown["execution"]["state"] == "exited").then_some(shown)
    })
}

fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

#[test]
fn a_session_whose_saved_conversation_is_missing_cannot_resume_until_it_exists() {
    let sandbox = Sandbox::new();
    declare(
        &sandbox,
        "fake",
        json!({
            "resume_transcript": {
                "env": "FAKE_TRANSCRIPT_DIR", "dir": "{home}/.fake-transcripts", "glob": "sessions/*/{ref}.log"
            }
        }),
    );
    declare(&sandbox, "plain", json!({ "resume": null }));
    let user_home = fake_user_home(&sandbox);
    let _service = spawn_service(&sandbox, &user_home, &[]);
    add_workspace(&sandbox);

    let session = run_and_stop(&sandbox, "fake");
    let id = session["id"].as_str().unwrap().to_owned();
    let agent_ref = session["agent_ref"]
        .as_str()
        .expect("the assigned ref")
        .to_owned();
    assert_eq!(
        session["resume"],
        json!({ "available": false, "code": "agent.resume_not_found", "reason": NOT_FOUND_REASON })
    );
    let text = stdout_text(&sandbox.run("default", &["agent", "status", &id]).success());
    assert!(
        text.contains(&format!(
            "resume: unavailable: {NOT_FOUND_REASON} (agent.resume_not_found)"
        )),
        "{text}"
    );
    let refused = error_of(&sandbox, &["agent", "resume", &id]);
    assert_eq!(refused["code"], "agent.resume_not_found", "{refused}");

    // Once the CLI's file for the ref is there, the session resumes.
    let saved = user_home
        .join(".fake-transcripts")
        .join("sessions")
        .join("p1");
    fs::create_dir_all(&saved).unwrap();
    fs::write(saved.join(format!("{agent_ref}.log")), "").unwrap();
    let shown = sandbox.json_ok("default", &["agent", "status", &id]);
    assert_eq!(shown["resume"], json!({ "available": true }));
    let resumed = sandbox.json_ok("default", &["agent", "resume", &id]);
    assert_eq!(resumed["execution"]["state"], "live", "{resumed}");
    sandbox.json_ok("default", &["agent", "stop", &id, "--force"]);
    exited(&sandbox, &id);

    // A kind with no resume form says so, with the code `agent resume` refuses with.
    let plain = run_and_stop(&sandbox, "plain");
    assert_eq!(plain["resume"]["available"], false);
    assert_eq!(plain["resume"]["code"], "agent.resume_unavailable");
    let refused = error_of(
        &sandbox,
        &["agent", "resume", plain["id"].as_str().unwrap()],
    );
    assert_eq!(refused["code"], "agent.resume_unavailable", "{refused}");
}

#[test]
fn a_resume_that_finds_no_conversation_leaves_the_session_exited_and_not_resumable() {
    let sandbox = Sandbox::new();
    declare(
        &sandbox,
        "fake",
        json!({ "resume_not_found_pattern": "No conversation found with session ID" }),
    );
    let user_home = fake_user_home(&sandbox);
    let _service = spawn_service(&sandbox, &user_home, &[("FAKE_AGENT_RESUME_MISSING", "1")]);
    add_workspace(&sandbox);

    let session = run_and_stop(&sandbox, "fake");
    let id = session["id"].as_str().unwrap().to_owned();
    assert_eq!(
        session["resume"],
        json!({ "available": true }),
        "no transcript check"
    );

    sandbox.json_ok("default", &["agent", "resume", &id]);
    let failed = eventually("the failed resume to be classified", || {
        let shown = exited(&sandbox, &id);
        (shown["resume"]["available"] == false).then_some(shown)
    });
    assert_eq!(
        failed["resume"],
        json!({ "available": false, "code": "agent.resume_not_found", "reason": NOT_FOUND_REASON })
    );
    assert_eq!(failed["execution"]["state"], "exited");
    assert_eq!(failed["execution"]["exit"]["reason"], "exited", "{failed}");
    assert_eq!(failed["execution"]["exit"]["code"], 1, "{failed}");
    let refused = error_of(&sandbox, &["agent", "resume", &id]);
    assert_eq!(refused["code"], "agent.resume_not_found", "{refused}");
}
