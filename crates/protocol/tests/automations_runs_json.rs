//! The automation run wire shapes (decision 0125): method, topic and capability names, a run with its usage, unset
//! options left out, and a state this build does not know decoding as `Unknown`.

use massorca_protocol::methods::automations::{
    self, AutomationRun, AutomationRunList, AutomationRunState, AutomationRunTrigger,
    AutomationRunUsage, AutomationRunsParams, AutomationUsageStatus,
};
use massorca_protocol::{capabilities, codes};
use serde_json::json;

#[test]
fn the_run_methods_topic_capability_and_codes_have_their_documented_names() {
    assert_eq!(automations::RUN_NOW, "automation.run_now");
    assert_eq!(automations::RUNS, "automation.runs");
    assert_eq!(automations::TOPIC_AUTOMATION_CHANGED, "automation.changed");
    assert_eq!(capabilities::AUTOMATION_RUNS, "automation-runs/1");
    assert_eq!(codes::AUTOMATION_NO_PROJECT, "automation.no_project");
    assert_eq!(codes::AUTOMATION_RUN_MISSED, "automation.run_missed");
    assert_eq!(
        codes::AUTOMATION_RUN_INTERRUPTED,
        "automation.run_interrupted"
    );
    assert_eq!(codes::AUTOMATION_RUN_EXITED, "automation.run_exited");
    assert_eq!(codes::AUTOMATION_RUN_STRANDED, "automation.run_stranded");
}

#[test]
fn runs_params_omit_an_unset_limit_and_cursor() {
    let params = AutomationRunsParams {
        automation: "nightly".to_owned(),
        limit: None,
        before: None,
    };
    assert_eq!(
        serde_json::to_value(&params).unwrap(),
        json!({"automation": "nightly"})
    );
}

#[test]
fn a_missed_run_uses_the_documented_names_and_omits_absent_options() {
    let run = AutomationRun {
        id: "r1".to_owned(),
        automation_id: "a1".to_owned(),
        number: 3,
        trigger: AutomationRunTrigger::Schedule,
        state: AutomationRunState::SkippedMissed,
        scheduled_for_ms: 100,
        occurrences: 2,
        outcome: None,
        session_id: None,
        workspace_id: None,
        reason: Some("missed".to_owned()),
        reason_code: Some(codes::AUTOMATION_RUN_MISSED.to_owned()),
        result: None,
        usage: AutomationRunUsage {
            status: AutomationUsageStatus::Unavailable,
            reason: Some("not run".to_owned()),
            input_tokens: None,
            output_tokens: None,
        },
        started_at_ms: None,
        ended_at_ms: Some(110),
        created_at_ms: 105,
    };
    let value = serde_json::to_value(&run).unwrap();
    assert_eq!(
        value,
        json!({
            "id": "r1",
            "automation_id": "a1",
            "number": 3,
            "trigger": "schedule",
            "state": "skipped_missed",
            "scheduled_for_ms": 100,
            "occurrences": 2,
            "reason": "missed",
            "reason_code": "automation.run_missed",
            "usage": {"status": "unavailable", "reason": "not run"},
            "ended_at_ms": 110,
            "created_at_ms": 105,
        })
    );
    let list: AutomationRunList = serde_json::from_value(json!({"runs": [value]})).unwrap();
    assert_eq!(list.runs, [run]);
    assert_eq!(list.next_before, None);
}

#[test]
fn a_state_or_trigger_this_build_does_not_know_decodes_as_unknown() {
    let state: AutomationRunState = serde_json::from_value(json!("queued_elsewhere")).unwrap();
    let trigger: AutomationRunTrigger = serde_json::from_value(json!("webhook")).unwrap();
    assert_eq!(state, AutomationRunState::Unknown);
    assert_eq!(trigger, AutomationRunTrigger::Unknown);
    assert!(AutomationRunState::Dispatched.in_progress());
    assert!(!AutomationRunState::Completed.in_progress());
}
