//! Permission modes (decision 0083): which mode a run gets, and how it reaches the agent.
//!
//! When nobody chose a mode, no mode flag is passed and nothing is typed: the vendor CLI starts in its own default, and
//! the session says so (`vendor_default`) instead of naming a mode it cannot vouch for.

use massorca_core_store::RunMode;
use massorca_protocol::codes;
use serde_json::json;

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
/// planned the same way, from its own request only, so it never inherits its parent's mode.
pub(crate) fn plan_start(decl: &KindDecl, requested: Option<&str>) -> Result<ModePlan, AgentError> {
    match requested {
        Some(id) => plan(decl, declared(decl, id)?),
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
/// A mode the kind no longer declares, or can no longer apply, gives the vendor's default.
pub(crate) fn plan_resume(
    decl: &KindDecl,
    requested: Option<&str>,
    last: &RunMode,
    pending: Option<&str>,
) -> Result<(ModePlan, Option<String>), AgentError> {
    if let Some(id) = requested {
        return Ok((plan(decl, declared(decl, id)?)?, None));
    }
    if let Some(mode) = pending.and_then(|id| decl.permission_mode(id)) {
        if mode.bypass {
            return Ok((ModePlan::vendor_default(decl), Some(mode.id.clone())));
        }
        return Ok((
            plan(decl, mode).unwrap_or_else(|_| ModePlan::vendor_default(decl)),
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
        .and_then(|mode| plan(decl, mode).ok())
        .unwrap_or_else(|| ModePlan::vendor_default(decl));
    Ok((plan, None))
}

/// How a chosen `mode` reaches a run of `decl`. Launch arguments are confirmed unless the vendor may fall back from the
/// mode (R3). A mode with only a live switch is typed at the first `idle`, which only a kind that reports one can do; for
/// any other kind it is `agent.mode_unavailable` (R5). A mode with neither is the vendor's default.
pub(crate) fn plan(decl: &KindDecl, mode: &PermissionModeDecl) -> Result<ModePlan, AgentError> {
    let chosen = |confirmed| RunMode {
        permission_mode: Some(mode.id.clone()),
        source: Some(SOURCE_CHOSEN.to_owned()),
        confirmed,
    };
    if !mode.args.is_empty() {
        return Ok(ModePlan {
            mode: chosen(!mode.may_fall_back),
            args: mode.args.clone(),
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

/// Whether a resume could apply `mode`: it has launch arguments, or a live switch the first `idle` can carry (R9).
pub(crate) fn applies_at_resume(
    decl: &KindDecl,
    mode: &PermissionModeDecl,
) -> Result<(), AgentError> {
    plan(decl, mode).map(drop)
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
