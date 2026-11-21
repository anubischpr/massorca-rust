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

#[test]
fn command_group_without_a_subcommand_exits_2() {
    massorca().arg("service").assert().code(2);
}

#[test]
fn invalid_profile_name_exits_2_with_its_code() {
    massorca()
        .args(["--profile", "Bad Name", "service", "run"])
        .assert()
        .code(2)
        .stderr(contains("usage.invalid_profile"));
}

#[test]
fn help_exits_0_and_lists_the_command_groups() {
    massorca()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("service"))
        .stdout(contains("config"))
        .stdout(contains("ext"));
}
