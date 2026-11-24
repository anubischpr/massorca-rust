//! The status rules (decisions 0052, 0053): what a hook report changes, and what a run's exit changes.
//!
//! Pure functions over stored values, so the rules are tested without a service. [`crate::Agents`] applies them under
//! its status lock and writes the result to the session row.

use std::collections::BTreeMap;

use massorca_core_store::StoredActivity;
use massorca_protocol::methods::agents::MAX_RESULT_BYTES;
use serde_json::Value;

use crate::event::{DoneOutcome, EventTarget};
use crate::kind::KindDecl;

pub const SOURCE_HOOK: &str = "hook";
pub const SOURCE_PROCESS: &str = "process";

/// What a kind declares about its hook reports: the event table, the result field and the ref field.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HookRules {
    pub events: BTreeMap<String, EventTarget>,
    pub result_field: Option<String>,
    pub learn: Option<String>,
}

impl HookRules {
    pub fn of(decl: &KindDecl) -> Self {
        Self {
            events: decl.status.events.clone(),
            result_field: decl.status.result_field.clone(),
            learn: decl
                .session_ref
                .as_ref()
                .and_then(|session_ref| session_ref.learn.clone()),
        }
    }
}

/// What one accepted hook report changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub activity: StoredActivity,
    /// The turn's result text, for a `done` report whose payload carries the declared field; truncated.
    pub result: Option<String>,
    /// The agent's own id, from the declared payload field.
    pub learned_ref: Option<String>,
}

/// Maps one hook event through the kind's table. `None` when the event is unknown or mapped to `ignore`: nothing
/// changes.
pub fn report(rules: &HookRules, event: &str, payload: Option<&str>, at_ms: i64) -> Option<Report> {
    let target = *rules.events.get(event)?;
    let (state, outcome) = match target {
        EventTarget::Ignore => return None,
        EventTarget::Working => ("working", None),
        EventTarget::Waiting => ("waiting", None),
        EventTarget::Idle => ("idle", None),
        EventTarget::Done(outcome) => ("done", Some(outcome)),
    };
    let payload = payload.and_then(|text| serde_json::from_str::<Value>(text).ok());
    let field = |name: &Option<String>| -> Option<String> {
        let name = name.as_deref()?;
        payload.as_ref()?.get(name)?.as_str().map(str::to_owned)
    };
    let result = if outcome.is_some() {
        field(&rules.result_field).map(|text| truncate(text, MAX_RESULT_BYTES))
    } else {
        None
    };
    Some(Report {
        activity: StoredActivity {
            state: state.to_owned(),
            outcome: outcome.map(|outcome| outcome.as_str().to_owned()),
            source: SOURCE_HOOK.to_owned(),
            at: at_ms,
            seen_at: None,
        },
        result,
        learned_ref: field(&rules.learn).filter(|id| !id.is_empty()),
    })
}

/// The activity after a run's exit: `done` with outcome `interrupted`, source `process`, when the run was `working` or
/// `waiting`; otherwise `None` and the activity stays.
pub fn on_exit(activity: Option<&StoredActivity>, at_ms: i64) -> Option<StoredActivity> {
    let activity = activity?;
    matches!(activity.state.as_str(), "working" | "waiting").then(|| StoredActivity {
        state: "done".to_owned(),
        outcome: Some(DoneOutcome::Interrupted.as_str().to_owned()),
        source: SOURCE_PROCESS.to_owned(),
        at: at_ms,
        seen_at: None,
    })
}

/// `text` cut to at most `max` bytes, on a character boundary.
pub fn truncate(mut text: String, max: usize) -> String {
    if text.len() > max {
        let mut end = max;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> HookRules {
        HookRules {
            events: serde_json::from_value(serde_json::json!({
                "start": "idle", "prompt": "working", "ask": "waiting",
                "stop": "done", "fail": "done:failure", "noise": "ignore"
            }))
            .unwrap(),
            result_field: Some("result".to_owned()),
            learn: Some("session_id".to_owned()),
        }
    }

    fn state(report: &Report) -> (&str, Option<&str>, &str) {
        (
            report.activity.state.as_str(),
            report.activity.outcome.as_deref(),
            report.activity.source.as_str(),
        )
    }

    #[test]
    fn events_map_through_the_kind_table() {
        let rules = rules();
        let at = |event| report(&rules, event, None, 7);
        assert_eq!(state(&at("start").unwrap()), ("idle", None, "hook"));
        assert_eq!(state(&at("prompt").unwrap()), ("working", None, "hook"));
        assert_eq!(state(&at("ask").unwrap()), ("waiting", None, "hook"));
        assert_eq!(
            state(&at("stop").unwrap()),
            ("done", Some("success"), "hook")
        );
        assert_eq!(
            state(&at("fail").unwrap()),
            ("done", Some("failure"), "hook")
        );
        assert_eq!(at("stop").unwrap().activity.at, 7);
        assert_eq!(at("noise"), None, "ignore changes nothing");
        assert_eq!(at("SessionEnd"), None, "an unknown event is ignored");
    }

    #[test]
    fn only_a_done_report_takes_the_result_field() {
        let rules = rules();
        let payload = r#"{"result":"all good"}"#;
        assert_eq!(
            report(&rules, "stop", Some(payload), 1).unwrap().result,
            Some("all good".to_owned())
        );
        assert_eq!(
            report(&rules, "prompt", Some(payload), 1).unwrap().result,
            None
        );
        assert_eq!(
            report(&rules, "stop", Some("not json"), 1).unwrap().result,
            None
        );
        assert_eq!(
            report(&rules, "stop", Some(r#"{"result":3}"#), 1)
                .unwrap()
                .result,
            None
        );
    }

    #[test]
    fn a_long_result_is_truncated_on_a_character_boundary() {
        let long = "é".repeat(MAX_RESULT_BYTES);
        let payload = serde_json::json!({ "result": long }).to_string();
        let result = report(&rules(), "stop", Some(&payload), 1)
            .unwrap()
            .result
            .unwrap();
        assert_eq!(result.len(), MAX_RESULT_BYTES);
        assert!(result.chars().all(|c| c == 'é'));
        assert_eq!(truncate("aé".to_owned(), 2), "a");
        assert_eq!(truncate("short".to_owned(), 16), "short");
    }

    #[test]
    fn the_declared_field_teaches_the_agent_ref() {
        let rules = rules();
        let payload = r#"{"session_id":"conv-1"}"#;
        assert_eq!(
            report(&rules, "start", Some(payload), 1)
                .unwrap()
                .learned_ref,
            Some("conv-1".to_owned())
        );
        assert_eq!(report(&rules, "start", None, 1).unwrap().learned_ref, None);
        let without = HookRules {
            learn: None,
            ..rules
        };
        assert_eq!(
            report(&without, "start", Some(payload), 1)
                .unwrap()
                .learned_ref,
            None
        );
    }

    #[test]
    fn an_exit_while_working_or_waiting_is_an_interrupted_turn() {
        for running in ["working", "waiting"] {
            let activity = StoredActivity {
                state: running.to_owned(),
                outcome: None,
                source: "hook".to_owned(),
                at: 1,
                seen_at: None,
            };
            let after = on_exit(Some(&activity), 9).expect("interrupted");
            assert_eq!(after.state, "done");
            assert_eq!(after.outcome.as_deref(), Some("interrupted"));
            assert_eq!(after.source, "process");
            assert_eq!(after.at, 9);
        }
        for settled in ["idle", "done"] {
            let activity = StoredActivity {
                state: settled.to_owned(),
                outcome: None,
                source: "hook".to_owned(),
                at: 1,
                seen_at: None,
            };
            assert_eq!(on_exit(Some(&activity), 9), None);
        }
        assert_eq!(on_exit(None, 9), None, "absent activity stays absent");
    }
}
