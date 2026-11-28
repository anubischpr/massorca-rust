use massorca_protocol::methods::terminals;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt::Debug;

/// Checks that `value` serializes to `expected` and that `expected` decodes back to `value`.
fn assert_wire<T>(value: &T, expected: &Value)
where
    T: Serialize + DeserializeOwned + Debug + PartialEq,
{
    assert_eq!(&serde_json::to_value(value).expect("serializes"), expected);
    let decoded: T = serde_json::from_value(expected.clone()).expect("deserializes");
    assert_eq!(&decoded, value);
}

fn sample_terminal() -> terminals::Terminal {
    terminals::Terminal {
        id: "t-1".to_owned(),
        workspace_id: "w-1".to_owned(),
        state: terminals::TerminalState::Live,
        title: Some("build".to_owned()),
        cols: 120,
        rows: 40,
        pid: Some(4242),
        shell: "/bin/bash".to_owned(),
        cwd: "/work/app/feat-x".to_owned(),
        created_at_ms: 1_767_225_600_000_i64,
        exit: None,
        holds_secrets: false,
        git_credential_guard: false,
    }
}

fn sample_terminal_json() -> Value {
    json!({
        "id": "t-1",
        "workspace_id": "w-1",
        "state": "live",
        "title": "build",
        "cols": 120,
        "rows": 40,
        "pid": 4242,
        "shell": "/bin/bash",
        "cwd": "/work/app/feat-x",
        "created_at_ms": 1_767_225_600_000_i64
    })
}

fn exited_terminal() -> terminals::Terminal {
    terminals::Terminal {
        state: terminals::TerminalState::Exited,
        title: None,
        pid: None,
        exit: Some(terminals::TerminalExit {
            code: Some(0),
            signal: None,
            reason: terminals::ExitReason::Exited,
            at_ms: 1_767_225_601_000_i64,
        }),
        ..sample_terminal()
    }
}

fn exited_terminal_json() -> Value {
    json!({
        "id": "t-1",
        "workspace_id": "w-1",
        "state": "exited",
        "cols": 120,
        "rows": 40,
        "shell": "/bin/bash",
        "cwd": "/work/app/feat-x",
        "created_at_ms": 1_767_225_600_000_i64,
        "exit": { "code": 0, "reason": "exited", "at_ms": 1_767_225_601_000_i64 }
    })
}

#[test]
fn method_names_follow_the_contract() {
    assert_eq!(terminals::CREATE, "terminal.create");
    assert_eq!(terminals::LIST, "terminal.list");
    assert_eq!(terminals::SHOW, "terminal.show");
    assert_eq!(terminals::ATTACH, "terminal.attach");
    assert_eq!(terminals::INPUT, "terminal.input");
    assert_eq!(terminals::RESIZE, "terminal.resize");
    assert_eq!(terminals::KILL, "terminal.kill");
    assert_eq!(terminals::CLOSE, "terminal.close");
}

#[test]
fn terminal_limits_match_the_contract() {
    assert_eq!(terminals::MAX_INPUT_BYTES, 1024 * 1024);
    assert_eq!(terminals::DEFAULT_COLS, 120);
    assert_eq!(terminals::DEFAULT_ROWS, 40);
    assert_eq!(terminals::MIN_COLS, 2);
    assert_eq!(terminals::MAX_COLS, 1000);
    assert_eq!(terminals::MIN_ROWS, 1);
    assert_eq!(terminals::MAX_ROWS, 500);
    assert_eq!(terminals::MAX_CHUNK_BYTES, 64 * 1024);
}

#[test]
fn create_params_omit_unset_options() {
    assert_wire(
        &terminals::TerminalCreateParams {
            workspace: "w-1".to_owned(),
            shell: None,
            cwd: None,
            env: BTreeMap::new(),
            cols: None,
            rows: None,
            title: None,
            request_id: None,
        },
        &json!({ "workspace": "w-1" }),
    );
}

#[test]
fn create_params_carry_every_option() {
    assert_wire(
        &terminals::TerminalCreateParams {
            workspace: "w-1".to_owned(),
            shell: Some(terminals::ShellSpec {
                program: "/bin/bash".to_owned(),
                args: vec!["-l".to_owned()],
            }),
            cwd: Some("src".to_owned()),
            env: BTreeMap::from([("LANG".to_owned(), "C.UTF-8".to_owned())]),
            cols: Some(100),
            rows: Some(30),
            title: Some("build".to_owned()),
            request_id: Some("r-1".to_owned()),
        },
        &json!({
            "workspace": "w-1",
            "shell": { "program": "/bin/bash", "args": ["-l"] },
            "cwd": "src",
            "env": { "LANG": "C.UTF-8" },
            "cols": 100,
            "rows": 30,
            "title": "build",
            "request_id": "r-1"
        }),
    );
}

#[test]
fn shell_spec_omits_empty_args() {
    assert_wire(
        &terminals::ShellSpec {
            program: "pwsh".to_owned(),
            args: Vec::new(),
        },
        &json!({ "program": "pwsh" }),
    );
}

#[test]
fn list_params_workspace_is_optional() {
    assert_wire(
        &terminals::TerminalListParams { workspace: None },
        &json!({}),
    );
    assert_wire(
        &terminals::TerminalListParams {
            workspace: Some("w-1".to_owned()),
        },
        &json!({ "workspace": "w-1" }),
    );
}

#[test]
fn terminal_list_has_the_documented_shape() {
    assert_wire(
        &terminals::TerminalList {
            terminals: vec![sample_terminal(), exited_terminal()],
        },
        &json!({ "terminals": [sample_terminal_json(), exited_terminal_json()] }),
    );
}

#[test]
fn terminal_ref_uses_the_terminal_key() {
    assert_wire(
        &terminals::TerminalRef {
            terminal: "t-1".to_owned(),
        },
        &json!({ "terminal": "t-1" }),
    );
}

#[test]
fn attach_params_omit_unset_options() {
    assert_wire(
        &terminals::TerminalAttachParams {
            terminal: "t-1".to_owned(),
            since: None,
            cols: None,
            rows: None,
            answers_queries: None,
        },
        &json!({ "terminal": "t-1" }),
    );
}

#[test]
fn attach_params_carry_resume_offset_and_size() {
    assert_wire(
        &terminals::TerminalAttachParams {
            terminal: "t-1".to_owned(),
            since: Some(1024),
            cols: Some(80),
            rows: Some(24),
            answers_queries: None,
        },
        &json!({ "terminal": "t-1", "since": 1024, "cols": 80, "rows": 24 }),
    );
}

#[test]
fn attach_params_declare_that_the_client_answers_queries() {
    assert_wire(
        &terminals::TerminalAttachParams {
            terminal: "t-1".to_owned(),
            since: None,
            cols: None,
            rows: None,
            answers_queries: Some(true),
        },
        &json!({ "terminal": "t-1", "answers_queries": true }),
    );
}

#[test]
fn attach_params_from_older_clients_decode_without_the_declaration() {
    let params: terminals::TerminalAttachParams = serde_json::from_value(json!({
        "terminal": "t-1",
        "since": 1024,
        "cols": 80,
        "rows": 24,
    }))
    .expect("an attach without answers_queries decodes");
    assert_eq!(params.answers_queries, None);
    assert_eq!(
        (params.since, params.cols, params.rows),
        (Some(1024), Some(80), Some(24))
    );
}

#[test]
fn resize_params_have_the_documented_shape() {
    assert_wire(
        &terminals::TerminalResizeParams {
            terminal: "t-1".to_owned(),
            cols: 100,
            rows: 30,
        },
        &json!({ "terminal": "t-1", "cols": 100, "rows": 30 }),
    );
}

#[test]
fn kill_params_default_force_to_false() {
    let params: terminals::TerminalKillParams =
        serde_json::from_value(json!({ "terminal": "t-1" })).expect("decodes");
    assert_eq!(
        params,
        terminals::TerminalKillParams {
            terminal: "t-1".to_owned(),
            force: false,
        }
    );
    assert_wire(&params, &json!({ "terminal": "t-1", "force": false }));
}

#[test]
fn states_and_reasons_use_snake_case_names() {
    assert_wire(&terminals::TerminalState::Live, &json!("live"));
    assert_wire(
        &terminals::TerminalState::Unverifiable,
        &json!("unverifiable"),
    );
    assert_wire(&terminals::TerminalState::Exited, &json!("exited"));
    assert_wire(&terminals::ExitReason::Exited, &json!("exited"));
    assert_wire(&terminals::ExitReason::Killed, &json!("killed"));
    assert_wire(
        &terminals::ExitReason::ServiceStopped,
        &json!("service_stopped"),
    );
    assert_wire(&terminals::AttachMode::Resume, &json!("resume"));
    assert_wire(&terminals::AttachMode::Snapshot, &json!("snapshot"));
}

#[test]
fn terminal_has_the_documented_shape() {
    assert_wire(&sample_terminal(), &sample_terminal_json());
}

#[test]
fn terminal_omits_unset_title_pid_and_exit() {
    assert_wire(&exited_terminal(), &exited_terminal_json());
}

#[test]
fn an_agent_terminal_says_it_has_the_git_credential_guard() {
    let mut guarded_json = sample_terminal_json();
    guarded_json["git_credential_guard"] = json!(true);
    assert_wire(
        &terminals::Terminal {
            git_credential_guard: true,
            ..sample_terminal()
        },
        &guarded_json,
    );
    assert_wire(&sample_terminal(), &sample_terminal_json());
}

#[test]
fn a_terminal_from_an_older_service_decodes_without_the_guard_flag() {
    let decoded: terminals::Terminal =
        serde_json::from_value(sample_terminal_json()).expect("decodes");
    assert!(!decoded.git_credential_guard);
}

#[test]
fn exit_omits_unset_code_and_signal() {
    assert_wire(
        &terminals::TerminalExit {
            code: None,
            signal: None,
            reason: terminals::ExitReason::Killed,
            at_ms: 1_767_225_601_000_i64,
        },
        &json!({ "reason": "killed", "at_ms": 1_767_225_601_000_i64 }),
    );
    assert_wire(
        &terminals::TerminalExit {
            code: None,
            signal: Some("SIGTERM".to_owned()),
            reason: terminals::ExitReason::Killed,
            at_ms: 1_767_225_601_000_i64,
        },
        &json!({ "signal": "SIGTERM", "reason": "killed", "at_ms": 1_767_225_601_000_i64 }),
    );
}

#[test]
fn attach_events_have_the_documented_shape() {
    assert_wire(
        &terminals::AttachEvent::Attached {
            terminal: sample_terminal(),
            mode: terminals::AttachMode::Resume,
            offset: 1024,
        },
        &json!({
            "kind": "attached",
            "terminal": sample_terminal_json(),
            "mode": "resume",
            "offset": 1024
        }),
    );
    assert_wire(
        &terminals::AttachEvent::Snapshot {
            offset: 0,
            cols: 120,
            rows: 40,
            len: 5000,
        },
        &json!({ "kind": "snapshot", "offset": 0, "cols": 120, "rows": 40, "len": 5000 }),
    );
    assert_wire(
        &terminals::AttachEvent::Resized {
            cols: 100,
            rows: 30,
        },
        &json!({ "kind": "resized", "cols": 100, "rows": 30 }),
    );
    assert_wire(
        &terminals::AttachEvent::Exited {
            terminal: exited_terminal(),
        },
        &json!({ "kind": "exited", "terminal": exited_terminal_json() }),
    );
}

#[test]
fn unknown_attach_event_kind_decodes_as_unknown_and_ignores_its_fields() {
    let event: terminals::AttachEvent =
        serde_json::from_value(json!({ "kind": "replay", "offset": 3, "future": true }))
            .expect("decodes");
    assert_eq!(event, terminals::AttachEvent::Unknown);
}

#[test]
fn unknown_enum_values_decode_as_unknown() {
    let state: terminals::TerminalState =
        serde_json::from_value(json!("archiving")).expect("decodes");
    assert_eq!(state, terminals::TerminalState::Unknown);
    let reason: terminals::ExitReason =
        serde_json::from_value(json!("vaporized")).expect("decodes");
    assert_eq!(reason, terminals::ExitReason::Unknown);
    let mode: terminals::AttachMode = serde_json::from_value(json!("replay")).expect("decodes");
    assert_eq!(mode, terminals::AttachMode::Unknown);
}

#[test]
fn unknown_fields_are_ignored() {
    let mut value = sample_terminal_json();
    value["future_field"] = json!({ "nested": 1 });
    let terminal: terminals::Terminal = serde_json::from_value(value).expect("decodes");
    assert_eq!(terminal, sample_terminal());
}

#[test]
fn terminal_details_keep_the_terminal_fields_flat_and_name_the_path_source() {
    let details = terminals::TerminalDetails {
        terminal: sample_terminal(),
        path_source: Some(terminals::TerminalPathSource::Registry),
    };
    let mut expected = sample_terminal_json();
    expected["path_source"] = json!("registry");
    assert_wire(&details, &expected);
}

#[test]
fn a_service_that_omits_the_path_source_decodes_as_none() {
    let details: terminals::TerminalDetails =
        serde_json::from_value(sample_terminal_json()).expect("decodes");
    assert_eq!(details.terminal, sample_terminal());
    assert_eq!(details.path_source, None);
}

#[test]
fn an_unknown_path_source_decodes_as_unknown() {
    let mut value = sample_terminal_json();
    value["path_source"] = json!("vaporized");
    let details: terminals::TerminalDetails = serde_json::from_value(value).expect("decodes");
    assert_eq!(
        details.path_source,
        Some(terminals::TerminalPathSource::Unknown)
    );
}
