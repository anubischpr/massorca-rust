//! The agent wire (`docs/architecture/agents.md`): field names, omitted optionals, defaults and unknown enum values.

use std::collections::BTreeMap;
use std::fmt::Debug;

use massorca_protocol::methods::agents::{
    self, Activity, ActivitySource, ActivityState, AgentKind, AgentKinds, AgentSession,
    AgentStartParams, Execution, KindCredentials, Outcome, StatusMechanism,
};
use massorca_protocol::methods::terminals::{ExitReason, TerminalExit, TerminalState};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

/// Checks that `value` serializes to `expected` and that `expected` decodes back to `value`.
fn assert_wire<T>(value: &T, expected: &Value)
where
    T: Serialize + DeserializeOwned + Debug + PartialEq,
{
    assert_eq!(&serde_json::to_value(value).expect("serializes"), expected);
    let decoded: T = serde_json::from_value(expected.clone()).expect("deserializes");
    assert_eq!(&decoded, value);
}

fn live_session() -> AgentSession {
    AgentSession {
        id: "s-1".to_owned(),
        kind: "claude".to_owned(),
        workspace_id: "w-1".to_owned(),
        title: None,
        read_only: false,
        parent_id: None,
        brought_back_at_ms: None,
        agent_ref: None,
        terminal_id: Some("t-1".to_owned()),
        created_at_ms: 1_767_225_600_000,
        execution: Execution {
            state: TerminalState::Live,
            exit: None,
        },
        activity: None,
        result: None,
    }
}

#[test]
fn method_names_match_the_wire_table() {
    let names = [
        (agents::KINDS, "agent.kinds"),
        (agents::START, "agent.start"),
        (agents::LIST, "agent.list"),
        (agents::SHOW, "agent.show"),
        (agents::STOP, "agent.stop"),
        (agents::FORGET, "agent.forget"),
        (agents::PROMPT, "agent.prompt"),
        (agents::RESUME, "agent.resume"),
        (agents::HOOK, "agent.hook"),
        (agents::WATCH, "agent.watch"),
        (agents::DELEGATE, "agent.delegate"),
        (agents::TREE, "agent.tree"),
        (agents::BRING_BACK, "agent.bring_back"),
        (agents::CREDENTIALS_IMPORT, "agent.credentials.import"),
        (agents::CREDENTIALS_SHOW, "agent.credentials.show"),
        (agents::CREDENTIALS_REMOVE, "agent.credentials.remove"),
    ];
    for (constant, expected) in names {
        assert_eq!(constant, expected);
    }
}

#[test]
fn a_live_session_omits_absent_fields() {
    assert_wire(
        &live_session(),
        &json!({
            "id": "s-1",
            "kind": "claude",
            "workspace_id": "w-1",
            "read_only": false,
            "terminal_id": "t-1",
            "created_at_ms": 1_767_225_600_000_i64,
            "execution": { "state": "live" }
        }),
    );
}

#[test]
fn an_exited_session_carries_its_exit_activity_and_result() {
    let session = AgentSession {
        title: Some("fix the build".to_owned()),
        read_only: true,
        parent_id: Some("s-0".to_owned()),
        brought_back_at_ms: Some(1_767_225_603_000),
        agent_ref: Some("ref-1".to_owned()),
        execution: Execution {
            state: TerminalState::Exited,
            exit: Some(TerminalExit {
                code: None,
                signal: None,
                reason: ExitReason::ServiceStopped,
                at_ms: 1_767_225_602_000,
            }),
        },
        activity: Some(Activity {
            state: ActivityState::Done,
            outcome: Some(Outcome::Interrupted),
            source: ActivitySource::Process,
            at_ms: 1_767_225_602_000,
        }),
        result: Some("all green".to_owned()),
        ..live_session()
    };
    assert_wire(
        &session,
        &json!({
            "id": "s-1",
            "kind": "claude",
            "workspace_id": "w-1",
            "title": "fix the build",
            "read_only": true,
            "parent_id": "s-0",
            "brought_back_at_ms": 1_767_225_603_000_i64,
            "agent_ref": "ref-1",
            "terminal_id": "t-1",
            "created_at_ms": 1_767_225_600_000_i64,
            "execution": {
                "state": "exited",
                "exit": { "reason": "service_stopped", "at_ms": 1_767_225_602_000_i64 }
            },
            "activity": {
                "state": "done",
                "outcome": "interrupted",
                "source": "process",
                "at_ms": 1_767_225_602_000_i64
            },
            "result": "all green"
        }),
    );
}

#[test]
fn a_kind_lists_its_mechanism_in_kebab_case_and_omits_no_problems() {
    let kind = AgentKind {
        id: "gentle-shell".to_owned(),
        name: "Gentle Shell".to_owned(),
        icon: "rose".to_owned(),
        owner: "core".to_owned(),
        installed: true,
        program_path: Some("/usr/local/bin/gentle-shell".to_owned()),
        resumable: false,
        status_mechanism: StatusMechanism::ClaudeSettings,
        credentials: KindCredentials {
            supported: false,
            imported: false,
        },
        problems: Vec::new(),
    };
    assert_wire(
        &AgentKinds { kinds: vec![kind] },
        &json!({ "kinds": [{
            "id": "gentle-shell",
            "name": "Gentle Shell",
            "icon": "rose",
            "owner": "core",
            "installed": true,
            "program_path": "/usr/local/bin/gentle-shell",
            "resumable": false,
            "status_mechanism": "claude-settings",
            "credentials": { "supported": false, "imported": false }
        }]}),
    );
}

#[test]
fn start_params_default_every_optional_field() {
    let decoded: AgentStartParams =
        serde_json::from_value(json!({ "workspace": "main", "kind": "fake" })).expect("decodes");
    assert_eq!(
        decoded,
        AgentStartParams {
            workspace: "main".to_owned(),
            kind: "fake".to_owned(),
            prompt: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            read_only: false,
            title: None,
            cols: None,
            rows: None,
            request_id: None,
        }
    );
}

#[test]
fn values_from_a_newer_peer_decode_as_unknown() {
    let activity: Activity = serde_json::from_value(json!({
        "state": "thinking", "outcome": "partial", "source": "telepathy", "at_ms": 1
    }))
    .expect("decodes");
    assert_eq!(activity.state, ActivityState::Unknown);
    assert_eq!(activity.outcome, Some(Outcome::Unknown));
    assert_eq!(activity.source, ActivitySource::Unknown);
    let mechanism: StatusMechanism = serde_json::from_value(json!("acp-stream")).expect("decodes");
    assert_eq!(mechanism, StatusMechanism::Unknown);
}
