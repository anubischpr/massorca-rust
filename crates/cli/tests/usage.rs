//! Command line mistakes exit 2.

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use predicates::str::contains;

fn massorca() -> Command {
    Command::new(cargo_bin("massorca"))
}

#[test]
fn unknown_command_exits_2() {
    massorca()
        .arg("frobnicate")
        .assert()
        .code(2)
        .stderr(contains("unrecognized subcommand"));
}

#[test]
fn running_without_arguments_exits_2_and_prints_usage() {
    massorca().assert().code(2).stderr(contains("Usage"));
}
