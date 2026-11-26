//! `massorca marketplace services ...` (SKL-35) against a sandboxed service whose `PATH` is one temporary folder. GitLab
//! runs a fake `glab` script there that records its args and stdin; no test runs a real `glab` or reaches a network.

mod harness;

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::assert::Assert;
use harness::{Sandbox, single_document, with_json};
use massorca_process::test_executable::write_executable;
use tempfile::TempDir;

const TOKEN: &str = "cli-service-token-6b2f0d";

/// A sandbox whose service sees `bin` as its whole `PATH`, and never the registry's.
fn sandbox(bin: &Path) -> Sandbox {
    let sandbox = Sandbox::new();
    sandbox.set_env("PATH", bin.to_str().expect("utf-8 path"));
    sandbox.set_env("MASSORCA_VENDOR_PATH", "process");
    sandbox
}

/// A fake `glab` that appends its args and stdin next to itself. When `reject` exists there it repeats what it read on
/// stdin to stderr, as a careless program might, and exits 1.
fn fake_glab(bin: &Path) {
    if cfg!(windows) {
        let script = "@echo off\r\necho %*>>\"%~dp0args.txt\"\r\n\"%SystemRoot%\\System32\\findstr.exe\" \"^\" >>\"%~dp0stdin.txt\"\r\nif exist \"%~dp0reject\" (type \"%~dp0stdin.txt\" 1>&2 & exit /b 1)\r\nexit /b 0\r\n";
        fs::write(bin.join("glab.cmd"), script).expect("the fake glab");
    } else {
        let script = "#!/bin/sh\nd=${0%/*}\nprintf '%s\\n' \"$*\" >>\"$d/args.txt\"\nwhile IFS= read -r l || [ -n \"$l\" ]; do printf '%s\\n' \"$l\"; done >>\"$d/stdin.txt\"\nif [ -e \"$d/reject\" ]; then while IFS= read -r l; do printf '%s\\n' \"$l\" >&2; done <\"$d/stdin.txt\"; exit 1; fi\nexit 0\n";
        write_executable(&bin.join("glab"), script);
    }
}

fn assert_token_absent(outcome: &Assert) {
    let output = outcome.get_output();
    for stream in [&output.stdout, &output.stderr] {
        let text = String::from_utf8_lossy(stream);
        assert!(!text.contains(TOKEN), "the output shows the token: {text}");
    }
}

/// No file of the profile home, its service log included, holds the token in clear.
fn assert_token_absent_from_files(dir: &Path) {
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let Ok(entries) = fs::read_dir(&next) else {
            continue;
        };
        for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(bytes) = fs::read(&path) {
                let text = String::from_utf8_lossy(&bytes);
                assert!(!text.contains(TOKEN), "{} holds the token", path.display());
            }
        }
    }
}

fn row(sandbox: &Sandbox, service: &str) -> serde_json::Value {
    let list = sandbox.json_ok("default", &["marketplace", "services", "list"]);
    assert!(!list.to_string().contains(TOKEN));
    list["services"]
        .as_array()
        .expect("services")
        .iter()
        .find(|row| row["service"] == service)
        .expect("the service is listed")
        .clone()
}

#[test]
fn gitlab_connects_through_glab_with_the_token_on_stdin_only() {
    let bin = TempDir::new().expect("a bin folder");
    fake_glab(bin.path());
    let sandbox = sandbox(bin.path());
    let args = [
        "marketplace",
        "services",
        "connect",
        "gitlab",
        "--base-url",
        "https://gitlab.example.com",
    ];
    let outcome = sandbox
        .run_with_input("default", &args, format!("{TOKEN}\n").as_bytes())
        .success();
    assert_token_absent(&outcome);

    // A batch file sees `=` arguments quoted; the quotes are cmd's transport, not part of the argument.
    let glab_args = fs::read_to_string(bin.path().join("args.txt"))
        .expect("glab ran")
        .replace('"', "");
    let glab_stdin = fs::read_to_string(bin.path().join("stdin.txt")).expect("glab read stdin");
    assert!(
        !glab_args.contains(TOKEN),
        "the token is in glab's args: {glab_args}"
    );
    assert!(
        glab_args.contains("auth login --hostname=gitlab.example.com --stdin"),
        "{glab_args}"
    );
    assert!(
        glab_stdin.contains(TOKEN),
        "glab did not get the token on stdin"
    );
    let gitlab = row(&sandbox, "gitlab");
    assert_eq!(
        (&gitlab["state"], &gitlab["source"]),
        (&"connected".into(), &"external".into())
    );

    // Disconnect starts no glab: signing glab out would sign the person out for everything else on the computer.
    let runs_before = fs::read_to_string(bin.path().join("args.txt")).expect("glab runs");
    let refused = sandbox
        .run(
            "default",
            &with_json(&["marketplace", "services", "disconnect", "gitlab"]),
        )
        .code(1);
    let error = &single_document(&refused)["error"];
    assert_eq!(error["code"], "marketplace.service_unavailable");
    assert_eq!(
        error["message"],
        "MassOrca does not sign you out of GitLab: glab keeps its own sign-in for everything on this computer. To sign out, run `glab auth logout` yourself."
    );
    let runs_after = fs::read_to_string(bin.path().join("args.txt")).expect("glab runs");
    assert_eq!(runs_after, runs_before, "disconnect started glab");
    assert_token_absent_from_files(sandbox.home());
}

#[test]
fn a_vault_service_connects_from_an_environment_variable_and_disconnects() {
    let bin = TempDir::new().expect("an empty bin folder");
    let sandbox = sandbox(bin.path());
    sandbox.set_env("GITEA_TEST_TOKEN", TOKEN);
    let args = [
        "marketplace",
        "services",
        "connect",
        "gitea",
        "--base-url",
        "https://gitea.example.com",
        "--token-env",
        "GITEA_TEST_TOKEN",
    ];
    let outcome = sandbox.run("default", &args).success();
    assert_token_absent(&outcome);
    let gitea = row(&sandbox, "gitea");
    assert_eq!(
        (&gitea["state"], &gitea["source"]),
        (&"connected".into(), &"stored".into())
    );
    assert_eq!(gitea["base_url"], "https://gitea.example.com");
    let gitlab = row(&sandbox, "gitlab");
    assert_eq!(gitlab["detail"], "glab is not installed");

    sandbox
        .run(
            "default",
            &["marketplace", "services", "disconnect", "gitea"],
        )
        .success();
    assert_eq!(row(&sandbox, "gitea")["state"], "not-connected");
    assert_token_absent_from_files(sandbox.home());
}

#[test]
fn a_token_is_never_an_argument_and_an_empty_one_is_refused() {
    let bin = TempDir::new().expect("an empty bin folder");
    let sandbox = sandbox(bin.path());
    sandbox
        .run(
            "default",
            &["marketplace", "services", "connect", "gitea", TOKEN],
        )
        .code(2);
    let empty = sandbox
        .run_with_input(
            "default",
            &with_json(&["marketplace", "services", "connect", "gitea"]),
            b"\n",
        )
        .code(1);
    assert_eq!(
        single_document(&empty)["error"]["code"],
        "marketplace.empty_token"
    );
}

#[test]
fn a_glab_that_repeats_the_token_on_stderr_leaks_it_nowhere() {
    let bin = TempDir::new().expect("a bin folder");
    fake_glab(bin.path());
    fs::write(bin.path().join("reject"), "").expect("marker");
    let sandbox = sandbox(bin.path());
    let refused = sandbox
        .run_with_input(
            "default",
            &with_json(&["marketplace", "services", "connect", "gitlab"]),
            format!(
                "{TOKEN}
"
            )
            .as_bytes(),
        )
        .code(1);
    assert_token_absent(&refused);
    assert_eq!(
        single_document(&refused)["error"]["code"],
        "marketplace.service_rejected"
    );
    let stdin = fs::read_to_string(bin.path().join("stdin.txt")).expect("glab read stdin");
    assert!(
        stdin.contains(TOKEN),
        "the fake glab got the token to repeat"
    );
    assert_token_absent_from_files(sandbox.home());
}
