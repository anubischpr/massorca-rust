//! `massorca cli`: listed in the help, and refused for a remote host before any PATH is read. These tests never run
//! `cli install` or `cli uninstall`, which would change the PATH of the machine running them.

use std::process::Output;

fn massorca(args: &[&str]) -> Output {
    massorca_process::command(assert_cmd::cargo::cargo_bin("massorca"))
        .args(args)
        .output()
        .expect("massorca runs")
}

#[test]
fn cli_help_lists_install_uninstall_and_status() {
    let output = massorca(&["cli", "--help"]);
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(output.status.success(), "{text}");
    for command in ["install", "uninstall", "status"] {
        assert!(
            text.lines()
                .any(|line| line.trim_start().starts_with(command)),
            "`{command}` is not listed in:\n{text}"
        );
    }
}

#[test]
fn cli_refuses_a_remote_host_before_it_reads_any_path() {
    let output = massorca(&["--host", "vps", "cli", "status"]);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("cli.host_not_supported"), "{stderr}");
}
