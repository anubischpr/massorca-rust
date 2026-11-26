//! The options `agent.set_options` changes on a session (decision 0171). A change is typed into a running terminal run when
//! the kind declares a live command for it; otherwise it waits for the session's next resume. This module works out which,
//! before anything is typed or stored, so a refused value changes nothing.

use massorca_protocol::codes;
use massorca_protocol::methods::agents::{AgentSetOptionsParams, OptionApplied, OptionsApplied};

use crate::kind::KindDecl;
use crate::sessions::{AgentError, resume_unavailable};

/// Options a session keeps in memory (decision 0171): the values `agent.set_options` chose for its next run, and the values
/// its live run started with. The service forgets them with the session, as it forgets its other state in memory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ChosenOptions {
    pub(crate) model: Option<String>,
    pub(crate) effort: Option<String>,
    pub(crate) fast: Option<bool>,
}

/// What one `agent.set_options` call does, worked out before anything is typed or stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Plan {
    /// The texts typed into the running agent, in order: a live model command, then a live fast toggle.
    pub(crate) typed: Vec<String>,
    /// Where each changed option took effect. An option that already had the value asked for is absent.
    pub(crate) applied: OptionsApplied,
    /// What the session keeps for its next run, once the call succeeds.
    pub(crate) chosen: ChosenOptions,
    /// The fast mode the running agent is in after a typed toggle; `None` when no toggle was typed.
    pub(crate) fast_typed: Option<bool>,
}

/// Works out what `request` does to a session of kind `decl`. `running` is whether its terminal run is live, and then
/// `started` holds the options that run started with (the ones it has now, after its live changes); a stopped session is
/// checked against `chosen`, what its next run keeps. `recorded_fast` is whether the live run was started with fast mode
/// on. Values are validated as `agent.start` validates them, so a refused value is an error and nothing is planned.
pub(crate) fn plan(
    decl: &KindDecl,
    session: &str,
    request: &AgentSetOptionsParams,
    started: &ChosenOptions,
    chosen: &ChosenOptions,
    running: bool,
    recorded_fast: bool,
) -> Result<Plan, AgentError> {
    if request.model.is_none() && request.effort.is_none() && request.fast.is_none() {
        return Err(AgentError::new(
            codes::REQUEST_INVALID_PARAMS,
            "name at least one option to change: model, effort or fast",
        ));
    }
    // The values the session would run with: the ones the call names, else the ones it runs or keeps. They are checked as
    // `agent.start` checks them, before anything is typed or stored.
    let base = if running { started } else { chosen };
    let model = request.model.as_deref().or(base.model.as_deref());
    let effort = request.effort.as_deref().or(base.effort.as_deref());
    let fast = request.fast.or(base.fast).unwrap_or(false);
    decl.option_args(model, effort)?;
    decl.fast_for(model, fast)?;

    let mut plan = Plan {
        typed: Vec::new(),
        applied: OptionsApplied::default(),
        chosen: chosen.clone(),
        fast_typed: None,
    };
    let mut next_resume = false;
    if let Some(model) = &request.model {
        // A run that has this model already gets nothing typed and reports nothing; the model is still kept for the next
        // resume.
        let has_it = running && started.model.as_deref() == Some(model.as_str());
        let live = decl
            .model
            .as_ref()
            .and_then(|model_decl| model_decl.live_text(model))
            .filter(|_| running && !has_it);
        match live {
            Some(text) => {
                plan.typed.push(text);
                plan.applied.model = Some(OptionApplied::Live);
            }
            None if has_it => {}
            None if chosen.model.as_deref() != Some(model.as_str()) => {
                plan.applied.model = Some(OptionApplied::NextResume);
                next_resume = true;
            }
            None => {}
        }
        plan.chosen.model = Some(model.clone());
    }
    if let Some(effort) = &request.effort {
        if chosen.effort.as_deref() != Some(effort.as_str()) {
            plan.applied.effort = Some(OptionApplied::NextResume);
            next_resume = true;
        }
        plan.chosen.effort = Some(effort.clone());
    }
    if let Some(on) = request.fast {
        // A running session is compared with the state its run was started in; a stopped one with what its next run keeps.
        let current = if running {
            recorded_fast
        } else {
            chosen.fast.unwrap_or(false)
        };
        // Fast mode is a toggle: its command is typed only when the running run is not in the state asked for.
        let toggle = decl
            .fast
            .as_ref()
            .and_then(|fast| fast.live.clone())
            .filter(|_| running && on != recorded_fast);
        match toggle {
            Some(command) => {
                plan.typed.push(command);
                plan.applied.fast = Some(OptionApplied::Live);
                plan.fast_typed = Some(on);
            }
            None if current != on => {
                plan.applied.fast = Some(OptionApplied::NextResume);
                next_resume = true;
            }
            None => {}
        }
        plan.chosen.fast = Some(on);
    }
    if next_resume && !decl.resumable() {
        return Err(resume_unavailable(
            session,
            &format!(
                "kind {} cannot resume it, and the change has no live command",
                decl.id
            ),
        ));
    }
    Ok(plan)
}

/// The options a session keeps after a call (decision 0171): `current`, with each option the call named replaced by its
/// planned value. The caller reads `current` and writes the result under one lock, so two calls that name different
/// options both keep theirs.
pub(crate) fn merged(
    current: ChosenOptions,
    request: &AgentSetOptionsParams,
    planned: &ChosenOptions,
) -> ChosenOptions {
    ChosenOptions {
        model: if request.model.is_some() {
            planned.model.clone()
        } else {
            current.model
        },
        effort: if request.effort.is_some() {
            planned.effort.clone()
        } else {
            current.effort
        },
        fast: if request.fast.is_some() {
            planned.fast
        } else {
            current.fast
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::KindDecl;
    use massorca_protocol::codes;
    use serde_json::json;

    fn claude() -> KindDecl {
        KindDecl::from_json(include_str!("../kinds/claude.json")).expect("claude parses")
    }

    /// A kind that takes a model with a live command, a fast mode, and efforts, and cannot resume.
    fn one_shot() -> KindDecl {
        KindDecl::from_json(
            &json!({
                "id": "fake", "name": "Fake", "program": ["fake"],
                "status": { "mechanism": "env-command", "events": { "stop": "done" } },
                "model": {
                    "flag": "--model", "live": "/model {value}",
                    "choices": [{ "id": "opus", "label": "Opus", "fast": true }, { "id": "haiku", "label": "Haiku" }]
                },
                "effort": { "flag": "--effort", "levels": [{ "id": "low", "label": "Low" }, { "id": "high", "label": "High" }] },
                "fast": { "settings": { "fastMode": true }, "live": "/fast", "toggle": true }
            })
            .to_string(),
        )
        .expect("the fake kind parses")
    }

    fn asks(
        model: Option<&str>,
        effort: Option<&str>,
        fast: Option<bool>,
    ) -> AgentSetOptionsParams {
        AgentSetOptionsParams {
            session: "s-1".to_owned(),
            model: model.map(str::to_owned),
            effort: effort.map(str::to_owned),
            fast,
        }
    }

    fn opus() -> ChosenOptions {
        ChosenOptions {
            model: Some("opus".to_owned()),
            ..ChosenOptions::default()
        }
    }

    fn refused(result: Result<Plan, AgentError>) -> &'static str {
        result.expect_err("refused").code
    }

    #[test]
    fn a_running_session_takes_its_model_by_the_live_command() {
        let plan = plan(
            &claude(),
            "s-1",
            &asks(Some("opus"), None, None),
            &ChosenOptions::default(),
            &ChosenOptions::default(),
            true,
            false,
        )
        .expect("planned");
        assert_eq!(plan.typed, ["/model opus"]);
        assert_eq!(plan.applied.model, Some(OptionApplied::Live));
        assert_eq!(plan.chosen.model.as_deref(), Some("opus"));
        assert_eq!(plan.fast_typed, None);
    }

    #[test]
    fn a_stopped_session_keeps_its_model_for_the_next_resume() {
        let plan = plan(
            &claude(),
            "s-1",
            &asks(Some("opus"), None, None),
            &ChosenOptions::default(),
            &ChosenOptions::default(),
            false,
            false,
        )
        .expect("planned");
        assert!(plan.typed.is_empty(), "{:?}", plan.typed);
        assert_eq!(plan.applied.model, Some(OptionApplied::NextResume));
        assert_eq!(plan.chosen.model.as_deref(), Some("opus"));
    }

    #[test]
    fn an_option_already_at_its_value_changes_nothing() {
        let plan = plan(
            &claude(),
            "s-1",
            &asks(Some("opus"), None, None),
            &opus(),
            &opus(),
            false,
            false,
        )
        .expect("planned");
        assert!(plan.typed.is_empty());
        assert_eq!(plan.applied, OptionsApplied::default());
    }

    #[test]
    fn a_live_request_for_the_model_the_run_has_sends_nothing() {
        // The run started on opus: asking for opus again types nothing and reports nothing, but the model is still kept.
        let plan = plan(
            &claude(),
            "s-1",
            &asks(Some("opus"), None, None),
            &opus(),
            &ChosenOptions::default(),
            true,
            false,
        )
        .expect("planned");
        assert!(plan.typed.is_empty(), "{:?}", plan.typed);
        assert_eq!(plan.applied.model, None, "{plan:?}");
        assert_eq!(plan.chosen.model.as_deref(), Some("opus"));
    }

    #[test]
    fn a_call_keeps_the_stored_options_it_does_not_name() {
        // Another call stored a model and fast mode while this one planned from an older snapshot.
        let current = ChosenOptions {
            model: Some("haiku".to_owned()),
            effort: None,
            fast: Some(true),
        };
        let planned = ChosenOptions {
            model: None,
            effort: Some("high".to_owned()),
            fast: None,
        };
        assert_eq!(
            merged(current, &asks(None, Some("high"), None), &planned),
            ChosenOptions {
                model: Some("haiku".to_owned()),
                effort: Some("high".to_owned()),
                fast: Some(true),
            }
        );
    }

    #[test]
    fn an_effort_waits_for_the_next_resume_even_while_the_run_is_live() {
        let plan = plan(
            &claude(),
            "s-1",
            &asks(None, Some("high"), None),
            &opus(),
            &opus(),
            true,
            false,
        )
        .expect("planned");
        assert!(plan.typed.is_empty(), "{:?}", plan.typed);
        assert_eq!(plan.applied.effort, Some(OptionApplied::NextResume));
        assert_eq!(plan.chosen.effort.as_deref(), Some("high"));
    }

    #[test]
    fn fast_is_typed_only_when_the_recorded_state_differs() {
        let switched = plan(
            &claude(),
            "s-1",
            &asks(None, None, Some(true)),
            &opus(),
            &opus(),
            true,
            false,
        )
        .expect("planned");
        assert_eq!(switched.typed, ["/fast"]);
        assert_eq!(switched.applied.fast, Some(OptionApplied::Live));
        assert_eq!(switched.fast_typed, Some(true));
        assert_eq!(switched.chosen.fast, Some(true));

        let unchanged = plan(
            &claude(),
            "s-1",
            &asks(None, None, Some(true)),
            &ChosenOptions {
                model: Some("opus".to_owned()),
                fast: Some(true),
                ..ChosenOptions::default()
            },
            &opus(),
            true,
            true,
        )
        .expect("planned");
        assert!(unchanged.typed.is_empty(), "{:?}", unchanged.typed);
        assert_eq!(unchanged.applied, OptionsApplied::default());
        assert_eq!(unchanged.fast_typed, None);
    }

    #[test]
    fn a_running_fast_mode_the_run_already_has_is_not_applied_again() {
        // The run was started in fast mode, which the session keeps no record of in `chosen`, so `chosen` says off.
        let plan = plan(
            &claude(),
            "s-1",
            &asks(None, None, Some(true)),
            &opus(),
            &ChosenOptions {
                model: Some("opus".to_owned()),
                fast: Some(false),
                ..ChosenOptions::default()
            },
            true,
            true,
        )
        .expect("planned");
        assert!(plan.typed.is_empty(), "{:?}", plan.typed);
        assert_eq!(plan.applied.fast, None, "the run has it already: {plan:?}");
        assert_eq!(plan.chosen.fast, Some(true), "the next resume keeps it on");
    }

    #[test]
    fn a_running_session_checks_fast_mode_against_the_model_its_run_has() {
        // The run started on opus; the session keeps no model for its next run.
        let plan = plan(
            &claude(),
            "s-1",
            &asks(None, None, Some(true)),
            &opus(),
            &ChosenOptions::default(),
            true,
            false,
        )
        .expect("planned, since opus takes fast mode");
        assert_eq!(plan.typed, ["/fast"]);
    }

    #[test]
    fn a_stopped_session_checks_fast_mode_against_the_model_its_next_run_keeps() {
        // The run started on opus, but the session would resume on the kind's default, which does not take fast mode.
        assert_eq!(
            refused(plan(
                &claude(),
                "s-1",
                &asks(None, None, Some(true)),
                &opus(),
                &ChosenOptions::default(),
                false,
                false,
            )),
            codes::AGENT_OPTION_UNSUPPORTED
        );
    }

    #[test]
    fn a_fast_mode_the_run_lacks_waits_for_the_next_resume_when_no_toggle_fits() {
        let plan = plan(
            &claude(),
            "s-1",
            &asks(None, None, Some(true)),
            &ChosenOptions::default(),
            &opus(),
            false,
            false,
        )
        .expect("planned");
        assert!(plan.typed.is_empty());
        assert_eq!(plan.applied.fast, Some(OptionApplied::NextResume));
        assert_eq!(plan.chosen.fast, Some(true));
    }

    #[test]
    fn a_refused_value_is_refused_before_anything_is_planned() {
        assert_eq!(
            refused(plan(
                &claude(),
                "s-1",
                &asks(None, Some("turbo"), None),
                &opus(),
                &opus(),
                true,
                false,
            )),
            codes::REQUEST_INVALID_PARAMS
        );
        assert_eq!(
            refused(plan(
                &one_shot(),
                "s-1",
                &asks(Some("haiku"), None, Some(true)),
                &ChosenOptions::default(),
                &ChosenOptions::default(),
                true,
                false,
            )),
            codes::AGENT_OPTION_UNSUPPORTED
        );
    }

    #[test]
    fn a_request_that_names_no_option_is_refused() {
        assert_eq!(
            refused(plan(
                &claude(),
                "s-1",
                &asks(None, None, None),
                &opus(),
                &opus(),
                true,
                false,
            )),
            codes::REQUEST_INVALID_PARAMS
        );
    }

    #[test]
    fn a_kind_that_cannot_resume_refuses_what_only_the_next_resume_would_change() {
        assert_eq!(
            refused(plan(
                &one_shot(),
                "s-1",
                &asks(None, Some("low"), None),
                &ChosenOptions::default(),
                &ChosenOptions::default(),
                false,
                false,
            )),
            codes::AGENT_RESUME_UNAVAILABLE
        );
        let live = plan(
            &one_shot(),
            "s-1",
            &asks(Some("opus"), None, None),
            &ChosenOptions::default(),
            &ChosenOptions::default(),
            true,
            false,
        )
        .expect("a live model change needs no resume");
        assert_eq!(live.typed, ["/model opus"]);
    }
}
