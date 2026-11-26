//! Permission modes (decision 0083): which mode a run gets, and how it reaches the agent.
//!
//! When nobody chose a mode, no mode flag is passed and nothing is typed: the vendor CLI starts in its own default, and
//! the session says so (`vendor_default`) instead of naming a mode it cannot vouch for.

use std::path::Path;

use massorca_core_store::RunMode;
use massorca_protocol::codes;
use serde_json::json;

use crate::instructions;
use crate::kind::{KindDecl, PermissionModeDecl};
use crate::sessions::AgentError;

/// `mode_source` of a run in the vendor's own default.
pub(crate) const SOURCE_VENDOR_DEFAULT: &str = "vendor_default";
/// `mode_source` of a run in a mode someone chose.
pub(crate) const SOURCE_CHOSEN: &str = "chosen";

/// How a run's mode reaches its agent, and what its session records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ModePlan {
    /// What the session records about the run's mode.
    pub mode: RunMode,
    /// Launch arguments that select it, after the kind's own.
    pub args: Vec<String>,
    /// A live switch typed at the run's first `idle` (D3), for a chosen mode without launch arguments.
    pub switch_at_idle: Option<String>,
}

impl ModePlan {
    /// No flag and nothing typed: the vendor's own default, recorded as such when the kind declares modes.
    pub(crate) fn vendor_default(decl: &KindDecl) -> Self {
        Self {
            mode: RunMode {
                permission_mode: None,
                source: (!decl.permission_modes.is_empty())
                    .then(|| SOURCE_VENDOR_DEFAULT.to_owned()),
                confirmed: false,
            },
            ..Self::default()
        }
    }

    /// Caller arguments after the mode's own may override it, so they leave it unconfirmed (R4).
    pub(crate) fn with_caller_args(mut self, caller_args: &[String]) -> Self {
        if !caller_args.is_empty() {
            self.mode.confirmed = false;
        }
        self
    }
}

/// The mode of a session's first run: `requested`, or the vendor's default when none is named. A delegated child is
/// planned the same way, from its own request only, so it never inherits its parent's mode. `program` is the kind's
/// resolved program, whose `--help` chooses the launch value of a mode that declares them (decision 0164); without one,
/// the first launch value is passed.
pub(crate) fn plan_start(
    decl: &KindDecl,
    requested: Option<&str>,
    program: Option<&Path>,
) -> Result<ModePlan, AgentError> {
    match requested {
        Some(id) => plan(decl, declared(decl, id)?, program),
        None => Ok(ModePlan::vendor_default(decl)),
    }
}

/// The mode of a resumed run (D6, R2), with the mode still pending after it:
/// - `requested`, which also ends any wait;
/// - else a mode `agent.set_mode` stored for this resume, unless it is a bypass: a pending bypass is used only when the
///   resume names it, so it stays pending and the run gets the vendor's default;
/// - else the last chosen mode unless it is a bypass;
/// - else the vendor's default.
///
/// A mode the kind no longer declares, or can no longer apply, gives the vendor's default. `program` is as in
/// [`plan_start`].
pub(crate) fn plan_resume(
    decl: &KindDecl,
    requested: Option<&str>,
    last: &RunMode,
    pending: Option<&str>,
    program: Option<&Path>,
) -> Result<(ModePlan, Option<String>), AgentError> {
    if let Some(id) = requested {
        return Ok((plan(decl, declared(decl, id)?, program)?, None));
    }
    if let Some(mode) = pending.and_then(|id| decl.permission_mode(id)) {
        if mode.bypass {
            return Ok((ModePlan::vendor_default(decl), Some(mode.id.clone())));
        }
        return Ok((
            plan(decl, mode, program).unwrap_or_else(|_| ModePlan::vendor_default(decl)),
            None,
        ));
    }
    let kept = last
        .permission_mode
        .as_deref()
        .filter(|_| last.source.as_deref() == Some(SOURCE_CHOSEN))
        .and_then(|id| decl.permission_mode(id))
        .filter(|mode| !mode.bypass);
    let plan = kept
        .and_then(|mode| plan(decl, mode, program).ok())
        .unwrap_or_else(|| ModePlan::vendor_default(decl));
    Ok((plan, None))
}

/// How a chosen `mode` reaches a run of `decl`. Launch arguments are confirmed unless the vendor may fall back from the
/// mode (R3). A mode with only a live switch is typed at the first `idle`, which only a kind that reports one can do; for
/// any other kind it is `agent.mode_unavailable` (R5). A mode with neither is the vendor's default. `program` chooses the
/// launch value of a mode that declares `launch_values` (decision 0164).
pub(crate) fn plan(
    decl: &KindDecl,
    mode: &PermissionModeDecl,
    program: Option<&Path>,
) -> Result<ModePlan, AgentError> {
    let chosen = |confirmed| RunMode {
        permission_mode: Some(mode.id.clone()),
        source: Some(SOURCE_CHOSEN.to_owned()),
        confirmed,
    };
    if !mode.args.is_empty() {
        return Ok(ModePlan {
            mode: chosen(!mode.may_fall_back),
            args: mode.args_for(&launch_value(decl, mode, program)),
            switch_at_idle: None,
        });
    }
    match &mode.live_switch {
        Some(switch) if decl.reports_idle() => Ok(ModePlan {
            mode: chosen(false),
            args: Vec::new(),
            switch_at_idle: Some(switch.text.clone()),
        }),
        Some(_) => Err(unappliable(decl, mode)),
        None => Ok(ModePlan::vendor_default(decl)),
    }
}

/// Whether a resume could apply `mode`: it has launch arguments, or a live switch the first `idle` can carry (R9). The
/// launch value does not decide that, so the CLI is not asked.
pub(crate) fn applies_at_resume(
    decl: &KindDecl,
    mode: &PermissionModeDecl,
) -> Result<(), AgentError> {
    plan(decl, mode, None).map(drop)
}

/// The launch value a run of `mode` passes (decision 0164). A mode without launch values needs no value, so the CLI is
/// not asked for its help; otherwise the help comes from the cached probe, when a program is given.
fn launch_value(decl: &KindDecl, mode: &PermissionModeDecl, program: Option<&Path>) -> String {
    if mode.launch_values.is_empty() {
        return String::new();
    }
    let help = program.and_then(|program| instructions::help_output(decl, program));
    choose_launch_value(mode, help.as_deref())
}

/// The first of `mode`'s launch values that `help` lists for the flag its value follows, else the first one (decision
/// 0164). Without a help text, or when it lists none of them, the first spelling is passed.
pub(crate) fn choose_launch_value(mode: &PermissionModeDecl, help: Option<&str>) -> String {
    let Some(first) = mode.launch_values.first() else {
        return String::new();
    };
    let flag = mode.value_flag().unwrap_or_default();
    help.and_then(|help| {
        mode.launch_values
            .iter()
            .find(|value| instructions::lists_choice(help, flag, value.as_str()))
    })
    .unwrap_or(first)
    .clone()
}

fn unappliable(decl: &KindDecl, mode: &PermissionModeDecl) -> AgentError {
    AgentError::new(
        codes::AGENT_MODE_UNAVAILABLE,
        format!(
            "permission mode {} of agent kind {} has no launch arguments, and the kind reports no idle state to switch to it on; start without it and switch once the run is up",
            mode.id, decl.id
        ),
    )
}

/// The declared mode `id`, or `agent.mode_unknown` naming the kind, the mode and the known ids (D7).
pub(crate) fn declared<'a>(
    decl: &'a KindDecl,
    id: &str,
) -> Result<&'a PermissionModeDecl, AgentError> {
    if let Some(mode) = decl.permission_mode(id) {
        return Ok(mode);
    }
    let known: Vec<&str> = decl
        .permission_modes
        .iter()
        .map(|mode| mode.id.as_str())
        .collect();
    let message = if known.is_empty() {
        format!(
            "agent kind {} declares no permission modes, so it cannot run in {id}",
            decl.id
        )
    } else {
        format!(
            "agent kind {} has no permission mode {id}; its modes are {}",
            decl.id,
            known.join(", ")
        )
    };
    let mut error = AgentError::new(codes::AGENT_MODE_UNKNOWN, message);
    error.data = Some(json!({ "kind": decl.id, "mode": id, "known": known }));
    Err(error)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn kind(status: serde_json::Value) -> KindDecl {
        KindDecl::from_json(
            &json!({
                "id": "fake", "name": "Fake", "program": ["fake"], "status": status,
                "permission_modes": [
                    { "id": "careful", "label": "Careful", "args": ["--mode", "careful"] },
                    {
                        "id": "wild", "label": "Wild", "args": ["--mode", "wild"], "bypass": true,
                        "live_switch": { "type": "input", "text": "/mode wild" }
                    },
                    { "id": "quiet", "label": "Quiet", "live_switch": { "type": "input", "text": "/mode quiet" } },
                    { "id": "hopeful", "label": "Hopeful", "args": ["--mode", "hopeful"], "may_fall_back": true }
                ],
                "default_permission_mode": "careful"
            })
            .to_string(),
        )
        .unwrap()
    }

    fn idle_kind() -> KindDecl {
        kind(json!({ "mechanism": "env-command", "events": { "start": "idle" } }))
    }

    /// A kind whose Manual mode passes `--permission-mode` with the value its help lists, `default` or `manual`.
    fn manual_kind() -> KindDecl {
        KindDecl::from_json(
            &json!({
                "id": "claude", "name": "Claude", "program": ["claude"],
                "permission_modes": [{
                    "id": "default", "label": "Manual",
                    "args": ["--permission-mode", "{value}"],
                    "launch_values": ["default", "manual"]
                }],
                "default_permission_mode": "default"
            })
            .to_string(),
        )
        .unwrap()
    }

    fn chosen(id: &str, confirmed: bool) -> RunMode {
        RunMode {
            permission_mode: Some(id.to_owned()),
            source: Some(SOURCE_CHOSEN.to_owned()),
            confirmed,
        }
    }

    fn vendor() -> RunMode {
        RunMode {
            permission_mode: None,
            source: Some(SOURCE_VENDOR_DEFAULT.to_owned()),
            confirmed: false,
        }
    }

    #[test]
    fn no_mode_named_passes_nothing_and_records_the_vendor_default() {
        let plan = plan_start(&idle_kind(), None, None).unwrap();
        assert_eq!(plan.mode, vendor());
        assert!(plan.args.is_empty());
        assert_eq!(plan.switch_at_idle, None);
        let bare =
            KindDecl::from_json(r#"{"id":"bare","name":"Bare","program":["bare"]}"#).unwrap();
        assert_eq!(
            plan_start(&bare, None, None).unwrap(),
            ModePlan::default(),
            "no modes, no source"
        );
    }

    #[test]
    fn a_named_mode_with_arguments_is_passed_and_confirmed() {
        let plan = plan_start(&idle_kind(), Some("wild"), None).unwrap();
        assert_eq!(plan.args, ["--mode", "wild"]);
        assert_eq!(
            plan.mode,
            chosen("wild", true),
            "launch arguments win over the live switch"
        );
    }

    #[test]
    fn a_mode_the_vendor_may_fall_back_from_is_never_confirmed() {
        let plan = plan_start(&idle_kind(), Some("hopeful"), None).unwrap();
        assert_eq!(plan.args, ["--mode", "hopeful"]);
        assert_eq!(plan.mode, chosen("hopeful", false));
    }

    #[test]
    fn caller_arguments_leave_a_mode_unconfirmed() {
        let plan = plan_start(&idle_kind(), Some("careful"), None)
            .unwrap()
            .with_caller_args(&["--other".to_owned()]);
        assert_eq!(plan.mode, chosen("careful", false));
        let plain = plan_start(&idle_kind(), Some("careful"), None)
            .unwrap()
            .with_caller_args(&[]);
        assert!(plain.mode.confirmed);
    }

    #[test]
    fn a_mode_with_only_a_live_switch_is_typed_at_the_first_idle_unconfirmed() {
        let plan = plan_start(&idle_kind(), Some("quiet"), None).unwrap();
        assert!(plan.args.is_empty());
        assert_eq!(plan.switch_at_idle.as_deref(), Some("/mode quiet"));
        assert_eq!(plan.mode, chosen("quiet", false));
    }

    #[test]
    fn a_kind_without_idle_cannot_apply_a_switch_only_mode() {
        let silent = kind(json!({ "mechanism": "none" }));
        let error = plan_start(&silent, Some("quiet"), None).unwrap_err();
        assert_eq!(error.code, codes::AGENT_MODE_UNAVAILABLE);
        let quiet = silent.permission_mode("quiet").unwrap();
        assert!(applies_at_resume(&silent, quiet).is_err());
        assert!(applies_at_resume(&idle_kind(), quiet).is_ok());
    }

    #[test]
    fn a_resume_keeps_the_last_chosen_mode_unless_it_is_a_bypass() {
        let kind = idle_kind();
        let (kept, _) = plan_resume(&kind, None, &chosen("quiet", false), None, None).unwrap();
        assert_eq!(kept.mode, chosen("quiet", false));
        let (dropped, _) = plan_resume(&kind, None, &chosen("wild", true), None, None).unwrap();
        assert_eq!(dropped.mode, vendor(), "a bypass is never kept silently");
        let (named, pending) = plan_resume(
            &kind,
            Some("wild"),
            &chosen("careful", true),
            Some("quiet"),
            None,
        )
        .unwrap();
        assert_eq!(named.args, ["--mode", "wild"]);
        assert_eq!(pending, None, "a named mode ends the wait");
        let (gone, _) = plan_resume(&kind, None, &chosen("retired", true), None, None).unwrap();
        assert_eq!(gone.mode, vendor());
        let (default, _) = plan_resume(&kind, None, &vendor(), None, None).unwrap();
        assert_eq!(default.mode, vendor());
        let error = plan_resume(&kind, Some("reckless"), &vendor(), None, None).unwrap_err();
        assert_eq!(error.code, codes::AGENT_MODE_UNKNOWN);
    }

    #[test]
    fn a_pending_mode_applies_at_resume_but_a_pending_bypass_only_when_named() {
        let kind = idle_kind();
        let (careful, pending) =
            plan_resume(&kind, None, &chosen("wild", true), Some("careful"), None).unwrap();
        assert_eq!(careful.mode, chosen("careful", true));
        assert_eq!(pending, None);
        let (unnamed, pending) = plan_resume(&kind, None, &vendor(), Some("wild"), None).unwrap();
        assert_eq!(unnamed.mode, vendor());
        assert!(unnamed.args.is_empty());
        assert_eq!(pending.as_deref(), Some("wild"), "it stays pending");
        let (named, pending) =
            plan_resume(&kind, Some("wild"), &vendor(), Some("wild"), None).unwrap();
        assert_eq!(named.mode, chosen("wild", true));
        assert_eq!(pending, None);
    }

    #[test]
    fn an_unknown_mode_is_refused_with_the_known_ids() {
        let error = plan_start(&idle_kind(), Some("reckless"), None).unwrap_err();
        assert_eq!(error.code, codes::AGENT_MODE_UNKNOWN);
        assert_eq!(
            error.data,
            Some(json!({
                "kind": "fake", "mode": "reckless", "known": ["careful", "wild", "quiet", "hopeful"]
            }))
        );
        let none =
            KindDecl::from_json(r#"{"id":"bare","name":"Bare","program":["bare"]}"#).unwrap();
        let error = plan_start(&none, Some("careful"), None).unwrap_err();
        assert_eq!(error.code, codes::AGENT_MODE_UNKNOWN);
        assert!(
            error.message.contains("declares no permission modes"),
            "{}",
            error.message
        );
    }

    #[test]
    fn a_launch_value_is_substituted_into_the_mode_arguments_without_a_help_text() {
        let decl = manual_kind();
        let plan = plan_start(&decl, Some("default"), None).unwrap();
        assert_eq!(plan.args, ["--permission-mode", "default"]);
        assert_eq!(plan.mode, chosen("default", true));
    }

    #[test]
    fn the_launch_value_is_the_first_spelling_the_help_lists() {
        let decl = manual_kind();
        let mode = decl.permission_mode("default").unwrap();
        let manual_only = "  --permission-mode <MODE>  Permission mode (choices: \"acceptEdits\", \"manual\", \"plan\")";
        assert_eq!(choose_launch_value(mode, Some(manual_only)), "manual");
        let both = "  --permission-mode <MODE>  Permission mode (choices: \"default\", \"manual\")";
        assert_eq!(
            choose_launch_value(mode, Some(both)),
            "default",
            "the first listed spelling is preferred"
        );
        let neither = "  --permission-mode <MODE>  Permission mode (choices: \"plan\")";
        assert_eq!(choose_launch_value(mode, Some(neither)), "default");
        assert_eq!(
            choose_launch_value(mode, None),
            "default",
            "an unreadable help keeps the first spelling"
        );
    }
}
