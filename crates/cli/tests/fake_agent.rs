//! The fake agent program, run with piped stdin. Its hook reports go to the program
//! named by `MASSORCA_BIN`; these tests name a program that does not exist, so they
//! check what the agent prints, the exit codes and the commits it makes, and that a
//! hook program that cannot start does not stop it.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Output, Stdio};

/// A name no program has, so every hook call fails to start and is ignored.
const MISSING_HOOK_PROGRAM: &str = "massorca-no-such-program-for-the-fake-agent";

/// Runs the fake agent in `cwd` with `args` and the extra `env`, feeding `stdin` then closing it.
fn run_agent(args: &[&str], stdin: &str, cwd: &Path, env: &[(&str, &Path)]) -> Output {
    let mut command = massorca_process::command(env!("CARGO_BIN_EXE_massorca-fake-agent"));
    command
        .args(args)
        .current_dir(cwd)
        .env("MASSORCA_BIN", MISSING_HOOK_PROGRAM)
        .env_remove("FAKE_AGENT_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command.spawn().expect("start the fake agent");
    let mut input = child.stdin.take().expect("the agent's stdin");
    // The agent may exit before it reads all of its input (crash), so a broken pipe is not a failure here.
    let _ = input.write_all(stdin.as_bytes());
    drop(input);
    child.wait_with_output().expect("wait for the fake agent")
}

/// Runs the fake agent in a new temporary directory.
fn run_here(args: &[&str], stdin: &str) -> Output {
    let dir = tempfile::tempdir().expect("temp dir");
    run_agent(args, stdin, dir.path(), &[])
}

/// Runs the fake agent with `home` as its credential home.
fn run_with_home(home: &Path, stdin: &str) -> Output {
    let dir = tempfile::tempdir().expect("temp dir");
    run_agent(&[], stdin, dir.path(), &[("FAKE_AGENT_HOME", home)])
}

/// The printed lines, without their line endings.
fn lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The lines for `texts`, each with the `fake-agent: ` prefix.
fn expected(texts: &[&str]) -> Vec<String> {
    texts
        .iter()
        .map(|text| format!("fake-agent: {text}"))
        .collect()
}

/// The lines of a run without a session: `ready`, `credentials missing`, then `texts`.
fn started(texts: &[&str]) -> Vec<String> {
    let mut all = vec!["ready", "credentials missing"];
    all.extend_from_slice(texts);
    expected(&all)
}

/// Runs git in `dir` and returns its stdout; panics on failure.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = massorca_process::command("git")
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A repository with one empty commit, so the agent's commits have a parent.
fn repository_with_one_commit() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "commit.gpgsign", "false"]);
    git(dir.path(), &["config", "user.name", "t"]);
    git(dir.path(), &["config", "user.email", "t@example.invalid"]);
    git(
        dir.path(),
        &["commit", "-q", "--allow-empty", "-m", "initial commit"],
    );
    dir
}

#[test]
fn start_lines_then_exit_says_bye_and_exits_zero() {
    let output = run_here(&["--session-id", "S1"], "exit\n");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        lines(&output),
        expected(&["ready", "session S1", "credentials missing", "bye"])
    );
}

#[test]
fn resume_is_reported_after_the_session_line() {
    let output = run_here(&["--session-id", "S1", "--resume", "R1"], "exit\n");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        lines(&output),
        expected(&[
            "ready",
            "session S1",
            "resumed R1",
            "credentials missing",
            "bye"
        ])
    );
}

#[test]
fn a_prompt_from_stdin_runs_to_done() {
    let output = run_here(&[], "say hello\nexit\n");
    assert_eq!(
        lines(&output),
        started(&["working on say hello", "done say hello", "bye"])
    );
}

#[test]
fn a_prompt_containing_fail_reports_failed() {
    let output = run_here(&[], "please fail now\nexit\n");
    assert_eq!(
        lines(&output),
        started(&[
            "working on please fail now",
            "failed please fail now",
            "bye"
        ])
    );
}

#[test]
fn ask_answered_yes_continues_to_done() {
    let output = run_here(&[], "ask: deploy\nyes\nexit\n");
    assert_eq!(
        lines(&output),
        started(&[
            "working on ask: deploy",
            "question: may I continue? (yes/no)",
            "done ask: deploy",
            "bye"
        ])
    );
}

#[test]
fn ask_answered_no_is_declined_and_input_is_read_on() {
    let output = run_here(&[], "ask: deploy\nno\nexit\n");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        lines(&output),
        started(&[
            "working on ask: deploy",
            "question: may I continue? (yes/no)",
            "declined",
            "bye"
        ])
    );
}

#[test]
fn hold_waits_for_release_before_finishing() {
    let output = run_here(&[], "hold here\nrelease\nexit\n");
    assert_eq!(
        lines(&output),
        started(&[
            "working on hold here",
            "holding",
            "released",
            "done hold here",
            "bye"
        ])
    );
}

#[test]
fn write_adds_a_file_and_commits_it_in_the_working_directory() {
    let repo = repository_with_one_commit();
    let output = run_agent(&[], "write:notes.txt\nexit\n", repo.path(), &[]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        lines(&output),
        started(&[
            "working on write:notes.txt",
            "committed notes.txt",
            "done write:notes.txt",
            "bye"
        ])
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("notes.txt")).expect("the written file"),
        "written by fake agent\n"
    );
    assert_eq!(
        git(repo.path(), &["log", "-1", "--format=%s"]).trim(),
        "fake agent: notes.txt"
    );
    assert_eq!(
        git(repo.path(), &["log", "-1", "--format=%an <%ae>"]).trim(),
        "Fake <fake@example.invalid>"
    );
}

#[test]
fn a_write_name_outside_the_allowed_characters_is_not_written() {
    let repo = repository_with_one_commit();
    let output = run_agent(&[], "write:a/b\nexit\n", repo.path(), &[]);
    assert_eq!(
        lines(&output),
        started(&["working on write:a/b", "done write:a/b", "bye"])
    );
    assert_eq!(
        git(repo.path(), &["rev-list", "--count", "HEAD"]).trim(),
        "1"
    );
}

#[test]
fn crash_exits_with_code_three_at_once() {
    let output = run_here(&[], "crash\n");
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(lines(&output), started(&[]));
}

#[test]
fn credentials_found_is_printed_and_the_token_is_not() {
    let home = tempfile::tempdir().expect("temp dir");
    fs::write(home.path().join("token.json"), r#"{"token":"fake-token"}"#)
        .expect("write token file");
    let output = run_with_home(home.path(), "exit\n");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        lines(&output),
        expected(&["ready", "credentials found", "bye"])
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("fake-token"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("fake-token"));
}

#[test]
fn a_directory_named_token_json_counts_as_credentials_missing() {
    let home = tempfile::tempdir().expect("temp dir");
    fs::create_dir(home.path().join("token.json")).expect("create directory");
    let output = run_with_home(home.path(), "exit\n");
    assert_eq!(
        lines(&output),
        expected(&["ready", "credentials missing", "bye"])
    );
}

#[test]
fn prompt_words_on_the_command_line_run_before_stdin() {
    let output = run_here(&["--session-id", "S1", "say", "hello"], "");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        lines(&output),
        expected(&[
            "ready",
            "session S1",
            "credentials missing",
            "prompt at launch",
            "working on say hello",
            "done say hello"
        ])
    );
}

#[test]
fn bracketed_paste_markers_carriage_returns_and_blank_lines_are_ignored() {
    let output = run_here(&[], "\x1b[200~say hi\x1b[201~\r\n\r\n   \r\nexit\r\n");
    assert_eq!(
        lines(&output),
        started(&["working on say hi", "done say hi", "bye"])
    );
}

#[test]
fn a_hook_program_that_cannot_start_does_not_stop_a_prompt() {
    let output = run_here(&[], "say hello\nexit\n");
    assert_eq!(output.status.code(), Some(0));
    assert!(lines(&output).contains(&"fake-agent: done say hello".to_owned()));
}

#[test]
fn the_launch_mode_is_printed_after_the_session_line() {
    let output = run_here(&["--session-id", "s-9", "--mode", "careful"], "exit\n");
    assert_eq!(
        lines(&output),
        expected(&[
            "ready",
            "session s-9",
            "mode careful",
            "credentials missing",
            "bye"
        ])
    );
}

#[test]
fn a_mode_switch_line_is_recorded_and_is_not_a_prompt() {
    let output = run_here(&["--mode", "careful"], "/mode wild\nexit\n");
    assert_eq!(
        lines(&output),
        expected(&[
            "ready",
            "mode careful",
            "credentials missing",
            "mode switched wild",
            "bye"
        ])
    );
}
