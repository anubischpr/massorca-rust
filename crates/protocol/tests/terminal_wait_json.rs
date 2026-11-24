//! The wire shape of `terminal.wait` (capability `terminal-wait/1`): its params, its result, its capability name, its
//! cap and the error code for a terminal closed during a wait.

use std::fmt::Debug;

use massorca_protocol::capabilities;
use massorca_protocol::codes;
use massorca_protocol::methods::terminals::{
    self, ExitReason, TerminalExit, TerminalState, TerminalWait, TerminalWaitCondition,
    TerminalWaitParams,
};
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

#[test]
fn the_method_capability_cap_and_gone_code_are_named() {
    assert_eq!(terminals::WAIT, "terminal.wait");
    assert_eq!(capabilities::TERMINAL_WAIT, "terminal-wait/1");
    assert_eq!(codes::TERMINAL_GONE, "terminal.gone");
    assert_eq!(terminals::MAX_WAIT_MS, 60_000);
}

#[test]
fn wait_params_carry_the_condition_and_an_optional_timeout() {
    assert_wire(
        &TerminalWaitParams {
            terminal: "t-1".to_owned(),
            condition: TerminalWaitCondition::Exit,
            timeout_ms: Some(5_000),
        },
        &json!({ "terminal": "t-1", "condition": "exit", "timeout_ms": 5000 }),
    );
    assert_wire(
        &TerminalWaitParams {
            terminal: "t-1".to_owned(),
            condition: TerminalWaitCondition::Exit,
            timeout_ms: None,
        },
        &json!({ "terminal": "t-1", "condition": "exit" }),
    );
}

#[test]
fn a_condition_this_build_does_not_know_is_refused_not_ignored() {
    let refused = serde_json::from_value::<TerminalWaitParams>(
        json!({ "terminal": "t-1", "condition": "tui-idle" }),
    );
    assert!(refused.is_err(), "an unknown condition must not decode");
}

#[test]
fn a_wait_result_reports_satisfied_with_the_exit_or_unsatisfied_with_the_live_state() {
    assert_wire(
        &TerminalWait {
            terminal: "t-1".to_owned(),
            condition: TerminalWaitCondition::Exit,
            satisfied: true,
            status: TerminalState::Exited,
            exit: Some(TerminalExit {
                code: Some(3),
                signal: None,
                reason: ExitReason::Exited,
                at_ms: 1_767_225_600_000,
            }),
        },
        &json!({
            "terminal": "t-1",
            "condition": "exit",
            "satisfied": true,
            "status": "exited",
            "exit": { "code": 3, "reason": "exited", "at_ms": 1767225600000_i64 }
        }),
    );
    assert_wire(
        &TerminalWait {
            terminal: "t-1".to_owned(),
            condition: TerminalWaitCondition::Exit,
            satisfied: false,
            status: TerminalState::Live,
            exit: None,
        },
        &json!({ "terminal": "t-1", "condition": "exit", "satisfied": false, "status": "live" }),
    );
}
