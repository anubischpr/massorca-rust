//! One tool's probe: present with a version, missing, hung, unparsable, failing, and a shell that is never started.

use std::time::Duration;

use massorca_core_tools::probe::{ToolSpec, parse_version_output, probe_tool};
use massorca_protocol::methods::service::{ToolKind, ToolState};

use crate::fixture::{env_with_path, runs, stand_in, versioned};

const SECOND: Duration = Duration::from_secs(5);

fn gh() -> ToolSpec {
    ToolSpec {
        id: "gh",
        kind: ToolKind::Forge,
        program: "gh",
        version_args: Some(&["--version"]),
    }
}

#[test]
fn a_present_tool_reports_its_version_and_its_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let log = dir.path().join("runs.log");
    let program = versioned(dir.path(), "gh", "gh version 2.40.1 (2023-12-13)", &log);

    let info = probe_tool(&gh(), &env_with_path(dir.path()), SECOND);

    assert_eq!(info.id, "gh");
    assert_eq!(info.kind, ToolKind::Forge);
    assert_eq!(info.state, ToolState::Present);
    assert_eq!(info.version.as_deref(), Some("2.40.1"));
    // Why: Windows reports the PATHEXT spelling of the file (`gh.CMD`), which only differs in case.
    let expected = program.to_str().expect("utf-8 path");
    assert!(
        info.path
            .as_deref()
            .is_some_and(|path| path.eq_ignore_ascii_case(expected)),
        "{:?} is not {expected}",
        info.path
    );
    assert_eq!(runs(&log), 1);
}

#[test]
fn a_tool_not_on_the_path_is_missing_and_nothing_runs() {
    let dir = tempfile::tempdir().expect("temp dir");

    let info = probe_tool(&gh(), &env_with_path(dir.path()), SECOND);

    assert_eq!(info.state, ToolState::Missing);
    assert_eq!(info.version, None);
    assert_eq!(info.path, None);
}

#[test]
fn a_tool_that_hangs_past_the_timeout_is_unknown() {
    let dir = tempfile::tempdir().expect("temp dir");
    stand_in(
        dir.path(),
        "gh",
        "exec sleep 30",
        "ping -n 30 127.0.0.1 >nul",
    );

    let info = probe_tool(
        &gh(),
        &env_with_path(dir.path()),
        Duration::from_millis(500),
    );

    assert_eq!(info.state, ToolState::Unknown);
    assert_eq!(info.version, None);
    assert!(info.path.is_some());
}

#[test]
fn output_with_no_version_is_unknown() {
    let dir = tempfile::tempdir().expect("temp dir");
    stand_in(dir.path(), "gh", "echo hello", "echo hello");

    let info = probe_tool(&gh(), &env_with_path(dir.path()), SECOND);

    assert_eq!(info.state, ToolState::Unknown);
    assert_eq!(info.version, None);
}

#[test]
fn a_tool_that_exits_with_failure_is_unknown() {
    let dir = tempfile::tempdir().expect("temp dir");
    stand_in(
        dir.path(),
        "gh",
        "echo 'gh version 2.40.1'\nexit 3",
        "echo gh version 2.40.1\r\nexit /b 3",
    );

    let info = probe_tool(&gh(), &env_with_path(dir.path()), SECOND);

    assert_eq!(info.state, ToolState::Unknown);
}

#[test]
fn a_shell_is_found_but_never_started() {
    let dir = tempfile::tempdir().expect("temp dir");
    let log = dir.path().join("runs.log");
    versioned(dir.path(), "bash", "GNU bash 5.2", &log);
    let shell = ToolSpec {
        id: "bash",
        kind: ToolKind::Shell,
        program: "bash",
        version_args: None,
    };

    let info = probe_tool(&shell, &env_with_path(dir.path()), SECOND);

    assert_eq!(info.kind, ToolKind::Shell);
    assert_eq!(info.state, ToolState::Present);
    assert_eq!(info.version, None);
    assert!(info.path.is_some());
    assert_eq!(runs(&log), 0);
}

#[test]
fn the_version_is_read_from_the_first_dotted_number() {
    assert_eq!(
        parse_version_output("git version 2.45.1.windows.1").as_deref(),
        Some("2.45.1")
    );
    assert_eq!(
        parse_version_output("PowerShell 7.4.1\n").as_deref(),
        Some("7.4.1")
    );
    assert_eq!(
        parse_version_output("glab version 1.36.0 (2023-01-01)").as_deref(),
        Some("1.36.0")
    );
}

#[test]
fn output_without_a_dotted_number_has_no_version() {
    assert_eq!(parse_version_output("hello"), None);
    assert_eq!(parse_version_output("released in 2024"), None);
    assert_eq!(parse_version_output(""), None);
}
