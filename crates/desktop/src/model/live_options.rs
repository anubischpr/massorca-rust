//! A session's model, effort and Fast as its composer's picker shows them, and the `agent.set_options` a pick asks for
//! (decision 0171). Free of GPUI, so the rules test without a window.

use massorca_protocol::methods::agents::{
    AgentKind, AgentSession, AgentSetOptionsParams, OptionApplied, OptionsApplied,
};
use massorca_ui::components::model_picker::Choice;

use super::agent_models::{DeclaredEffort, DeclaredModel, declared_models};

/// The tail of the note for options the service kept for the session's next resume.
pub const NEXT_RESUME: &str = "when the session resumes";

/// The model, effort and speed a session runs with, or that its next resume takes, by declared id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionOptions {
    /// A declared model, or the kind's own id when it declares none.
    pub model: String,
    pub effort: Option<String>,
    pub fast: bool,
}

/// The session's options on its kind's list: its stored model and effort where the kind still offers them, otherwise
/// the kind's default model and that model's default level.
pub fn session_options(kind: &AgentKind, record: &AgentSession) -> SessionOptions {
    let fast = record.fast.as_ref().is_some_and(|fast| fast.on);
    let declared = declared_models(kind);
    let stored_effort = record.meta.effort.as_deref();
    if declared.is_empty() {
        let levels = kind_levels(kind);
        let effort = pick_effort(&levels, stored_effort, kind_default_level(kind, &levels));
        return SessionOptions {
            model: kind.id.clone(),
            effort,
            fast,
        };
    }
    let model = record
        .meta
        .model
        .as_deref()
        .and_then(|id| declared.iter().find(|model| model.id == id))
        .or_else(|| declared.iter().find(|model| model.default))
        .or(declared.first());
    let Some(model) = model else {
        return SessionOptions {
            model: kind.id.clone(),
            effort: None,
            fast,
        };
    };
    SessionOptions {
        model: model.id.clone(),
        effort: pick_effort(&model.efforts, stored_effort, model.default_effort),
        fast,
    }
}

/// The picker choice that shows the session's options.
pub fn session_choice(kind: &AgentKind, record: &AgentSession) -> Choice {
    let options = session_options(kind, record);
    let efforts = efforts_of(kind, &options.model);
    Choice {
        effort: options
            .effort
            .as_deref()
            .and_then(|id| efforts.iter().position(|effort| effort.id == id)),
        model: options.model.into(),
        fast: options.fast,
    }
}

/// The `agent.set_options` a pick asks for: only the options that differ from the session's. `None` when nothing
/// changes, or when the pick names a model the session's kind does not offer.
pub fn options_to_set(
    kind: &AgentKind,
    record: &AgentSession,
    choice: &Choice,
) -> Option<AgentSetOptionsParams> {
    let current = session_options(kind, record);
    let declared = declared_models(kind);
    let picked_model = if declared.is_empty() {
        (choice.model.as_ref() == kind.id).then(|| kind.id.clone())?
    } else {
        declared
            .iter()
            .find(|model| model.id == choice.model.as_ref())
            .map(|model| model.id.clone())?
    };
    let picked_effort = choice.effort.and_then(|index| {
        efforts_of(kind, &picked_model)
            .get(index)
            .map(|effort| effort.id.clone())
    });
    let params = AgentSetOptionsParams {
        session: record.id.clone(),
        model: (!declared.is_empty() && picked_model != current.model).then_some(picked_model),
        effort: picked_effort.filter(|effort| current.effort.as_ref() != Some(effort)),
        fast: (choice.fast != current.fast).then_some(choice.fast),
    };
    (params.model.is_some() || params.effort.is_some() || params.fast.is_some()).then_some(params)
}

/// The note for the options the service kept for the next resume, e.g. "Model and effort apply when the session
/// resumes"; `None` when every change took effect at once.
pub fn applied_note(applied: &OptionsApplied) -> Option<String> {
    let waiting: Vec<&str> = [
        ("Model", applied.model),
        ("effort", applied.effort),
        ("Fast", applied.fast),
    ]
    .into_iter()
    .filter(|(_, how)| *how == Some(OptionApplied::NextResume))
    .map(|(name, _)| name)
    .collect();
    let (names, verb) = match waiting.as_slice() {
        [] => return None,
        [one] => (capitalised(one), "applies"),
        [first, second] => (format!("{} and {second}", capitalised(first)), "apply"),
        [first, middle @ .., last] => {
            let mut names = capitalised(first);
            for name in middle {
                names.push_str(", ");
                names.push_str(name);
            }
            (format!("{names} and {last}"), "apply")
        }
    };
    Some(format!("{names} {verb} {NEXT_RESUME}"))
}

fn capitalised(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn pick_effort(levels: &[DeclaredEffort], stored: Option<&str>, default: usize) -> Option<String> {
    stored
        .and_then(|id| levels.iter().find(|level| level.id == id))
        .or_else(|| levels.get(default))
        .or(levels.first())
        .map(|level| level.id.clone())
}

/// The levels `model` takes: the declared model's, or the kind's own when it declares no models.
fn efforts_of(kind: &AgentKind, model: &str) -> Vec<DeclaredEffort> {
    let declared = declared_models(kind);
    if declared.is_empty() {
        return kind_levels(kind);
    }
    declared
        .into_iter()
        .find(|declared: &DeclaredModel| declared.id == model)
        .map(|declared| declared.efforts)
        .unwrap_or_default()
}

fn kind_levels(kind: &AgentKind) -> Vec<DeclaredEffort> {
    kind.effort.as_ref().map_or_else(Vec::new, |effort| {
        effort
            .levels
            .iter()
            .map(|level| DeclaredEffort {
                id: level.id.clone(),
                label: level.label.clone(),
            })
            .collect()
    })
}

fn kind_default_level(kind: &AgentKind, levels: &[DeclaredEffort]) -> usize {
    kind.effort
        .as_ref()
        .and_then(|effort| effort.default.as_deref())
        .and_then(|id| levels.iter().position(|level| level.id == id))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::{agent, agent_kind, base_kind_model_choice};
    use massorca_protocol::methods::agents::{
        KindEffort, KindEffortLevel, KindModel, KindModelChoice, PackageState, SessionFast,
    };

    fn choice(id: &str, default: bool, fast: bool) -> KindModelChoice {
        KindModelChoice {
            id: id.into(),
            label: id.to_uppercase(),
            default,
            fast,
            ..base_kind_model_choice()
        }
    }

    fn claude() -> AgentKind {
        let mut kind = agent_kind("claude", "Claude", PackageState::Enabled);
        kind.model = Some(KindModel {
            choices: vec![choice("opus", false, true), choice("sonnet", true, false)],
        });
        kind.effort = Some(KindEffort {
            levels: ["low", "medium", "high"]
                .into_iter()
                .map(|id| KindEffortLevel {
                    id: id.into(),
                    label: id.into(),
                })
                .collect(),
            default: Some("medium".into()),
        });
        kind
    }

    fn running(model: Option<&str>, effort: Option<&str>) -> AgentSession {
        let mut record = agent("s1", "w1", 1);
        record.kind = "claude".into();
        record.meta.model = model.map(Into::into);
        record.meta.effort = effort.map(Into::into);
        record
    }

    fn pick(model: &str, effort: Option<usize>, fast: bool) -> Choice {
        Choice {
            model: model.to_owned().into(),
            effort,
            fast,
        }
    }

    #[test]
    fn the_picker_shows_the_stored_model_and_effort_and_the_run_fast_mode() {
        let mut record = running(Some("opus"), Some("high"));
        record.fast = Some(SessionFast {
            on: true,
            confirmed: false,
        });
        assert_eq!(
            session_choice(&claude(), &record),
            pick("opus", Some(2), true)
        );
    }

    #[test]
    fn a_session_started_without_options_shows_the_kind_defaults() {
        assert_eq!(
            session_choice(&claude(), &running(None, None)),
            pick("sonnet", Some(1), false)
        );
    }

    #[test]
    fn a_pick_sends_only_what_changed() {
        let record = running(Some("sonnet"), Some("medium"));
        let set =
            options_to_set(&claude(), &record, &pick("opus", Some(1), false)).expect("a change");
        assert_eq!(set.session, "s1");
        assert_eq!(set.model.as_deref(), Some("opus"));
        assert_eq!(set.effort, None, "the level did not change");
        assert_eq!(set.fast, None);

        let set =
            options_to_set(&claude(), &record, &pick("sonnet", Some(2), true)).expect("a change");
        assert_eq!(
            (set.model, set.effort.as_deref(), set.fast),
            (None, Some("high"), Some(true))
        );
    }

    #[test]
    fn picking_what_runs_or_another_kind_model_asks_for_nothing() {
        let record = running(Some("sonnet"), Some("medium"));
        assert_eq!(
            options_to_set(&claude(), &record, &pick("sonnet", Some(1), false)),
            None
        );
        assert_eq!(
            options_to_set(&claude(), &record, &pick("gpt-5", None, false)),
            None
        );
    }

    #[test]
    fn the_note_names_each_option_kept_for_the_next_resume() {
        let applied = |model, effort, fast| OptionsApplied {
            model,
            effort,
            fast,
        };
        let (live, later) = (Some(OptionApplied::Live), Some(OptionApplied::NextResume));
        assert_eq!(
            applied_note(&applied(live, None, live)),
            None,
            "live shows nothing"
        );
        assert_eq!(
            applied_note(&applied(live, later, None)).as_deref(),
            Some("Effort applies when the session resumes")
        );
        assert_eq!(
            applied_note(&applied(later, later, None)).as_deref(),
            Some("Model and effort apply when the session resumes")
        );
        assert_eq!(
            applied_note(&applied(later, later, later)).as_deref(),
            Some("Model, effort and Fast apply when the session resumes")
        );
    }
}
