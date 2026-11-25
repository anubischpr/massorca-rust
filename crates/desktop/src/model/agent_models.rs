//! A kind's declared models and their reasoning levels, as the model picker lists them (decisions 0094 and 0096). A model
//! accepts every level of its kind when it names none, and none when it names an empty list. Free of GPUI, so the rules
//! test without a window.

use massorca_protocol::methods::agents::{AgentKind, KindEffortLevel};

/// One reasoning level of a declared model: the id `agent.start` takes and the name the picker shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredEffort {
    pub id: String,
    pub label: String,
}

/// A model a kind declares, with the levels it accepts in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredModel {
    pub id: String,
    pub label: String,
    pub efforts: Vec<DeclaredEffort>,
    /// The index of the level the picker preselects: the kind's default level when this model accepts it, else the first.
    pub default_effort: usize,
    /// Whether the picker preselects this model.
    pub default: bool,
}

fn level(levels: &[KindEffortLevel], id: &str) -> Option<DeclaredEffort> {
    levels
        .iter()
        .find(|level| level.id == id)
        .map(|level| DeclaredEffort {
            id: level.id.clone(),
            label: level.label.clone(),
        })
}

/// The models a kind declares, in declared order. Empty when it declares none, and the picker then offers the kind itself.
pub fn declared_models(kind: &AgentKind) -> Vec<DeclaredModel> {
    let Some(model) = &kind.model else {
        return Vec::new();
    };
    let levels: &[KindEffortLevel] = kind
        .effort
        .as_ref()
        .map_or(&[], |effort| effort.levels.as_slice());
    let default_level = kind
        .effort
        .as_ref()
        .and_then(|effort| effort.default.as_deref());
    model
        .choices
        .iter()
        .map(|choice| {
            let efforts: Vec<DeclaredEffort> = match &choice.efforts {
                None => levels
                    .iter()
                    .map(|declared| DeclaredEffort {
                        id: declared.id.clone(),
                        label: declared.label.clone(),
                    })
                    .collect(),
                Some(accepted) => accepted
                    .iter()
                    .map(|id| {
                        level(levels, id).unwrap_or_else(|| DeclaredEffort {
                            id: id.clone(),
                            label: id.clone(),
                        })
                    })
                    .collect(),
            };
            let default_effort = default_level
                .and_then(|id| efforts.iter().position(|effort| effort.id == id))
                .unwrap_or(0);
            DeclaredModel {
                id: choice.id.clone(),
                label: choice.label.clone(),
                efforts,
                default_effort,
                default: choice.default,
            }
        })
        .collect()
}

/// The declared model `model` names, with the kind that declares it; `None` when no kind declares it.
pub fn declared_model<'a>(
    kinds: &'a [AgentKind],
    model: &str,
) -> Option<(&'a AgentKind, DeclaredModel)> {
    kinds.iter().find_map(|kind| {
        declared_models(kind)
            .into_iter()
            .find(|declared| declared.id == model)
            .map(|declared| (kind, declared))
    })
}

/// The kind a picker choice belongs to: the kind that declares the chosen model, or the kind itself when it declares no
/// models and is offered as its own row.
pub fn choice_kind<'a>(kinds: &'a [AgentKind], choice: &str) -> Option<&'a AgentKind> {
    declared_model(kinds, choice)
        .map(|(kind, _)| kind)
        .or_else(|| {
            kinds
                .iter()
                .find(|kind| kind.id == choice && declared_models(kind).is_empty())
        })
}

/// The model the picker preselects among `kinds`, in their order: the first declared default.
pub fn preselected_model(kinds: &[AgentKind]) -> Option<String> {
    kinds
        .iter()
        .flat_map(declared_models)
        .find(|model| model.default)
        .map(|model| model.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::agent_kind;
    use massorca_protocol::methods::agents::{
        KindEffort, KindModel, KindModelChoice, PackageState,
    };

    fn choice(id: &str, label: &str, default: bool, efforts: Option<&[&str]>) -> KindModelChoice {
        KindModelChoice {
            id: id.into(),
            label: label.into(),
            default,
            efforts: efforts.map(|list| list.iter().map(|id| (*id).to_owned()).collect()),
        }
    }

    fn level(id: &str, label: &str) -> KindEffortLevel {
        KindEffortLevel {
            id: id.into(),
            label: label.into(),
        }
    }

    /// Claude as `claude.json` declares it: Opus takes every level, Sonnet is the default and takes four, Haiku takes none.
    fn claude() -> AgentKind {
        let mut kind = agent_kind("claude", "Claude", PackageState::Enabled);
        kind.model = Some(KindModel {
            choices: vec![
                choice("opus", "Opus", false, None),
                choice(
                    "sonnet",
                    "Sonnet",
                    true,
                    Some(&["low", "medium", "high", "max"]),
                ),
                choice("haiku", "Haiku", false, Some(&[])),
            ],
        });
        kind.effort = Some(KindEffort {
            levels: vec![
                level("low", "Low"),
                level("medium", "Medium"),
                level("high", "High"),
                level("xhigh", "Extra high"),
                level("max", "Max"),
            ],
            default: Some("high".into()),
        });
        kind
    }

    fn labels(model: &DeclaredModel) -> Vec<&str> {
        model
            .efforts
            .iter()
            .map(|effort| effort.label.as_str())
            .collect()
    }

    #[test]
    fn a_kind_lists_its_declared_models_in_order_with_their_accepted_levels() {
        let models = declared_models(&claude());
        assert_eq!(
            models
                .iter()
                .map(|model| model.label.as_str())
                .collect::<Vec<_>>(),
            ["Opus", "Sonnet", "Haiku"]
        );
        assert_eq!(
            labels(&models[0]),
            ["Low", "Medium", "High", "Extra high", "Max"],
            "a model that names no levels takes them all"
        );
        assert_eq!(labels(&models[1]), ["Low", "Medium", "High", "Max"]);
        assert!(labels(&models[2]).is_empty(), "an empty list takes none");
    }

    #[test]
    fn a_model_preselects_the_kind_default_level_it_accepts() {
        let models = declared_models(&claude());
        assert_eq!(models[0].default_effort, 2, "High is the kind's default");
        assert_eq!(models[1].efforts[models[1].default_effort].id, "high");
        assert!(models[1].default && !models[0].default);
    }

    #[test]
    fn a_model_that_does_not_take_the_default_level_starts_on_its_first() {
        let mut kind = claude();
        kind.model = Some(KindModel {
            choices: vec![choice("sonnet", "Sonnet", true, Some(&["low", "max"]))],
        });
        let models = declared_models(&kind);
        assert_eq!(models[0].default_effort, 0);
    }

    #[test]
    fn the_effort_ids_are_the_declared_ids_whatever_their_labels() {
        let mut kind = claude();
        kind.effort = Some(KindEffort {
            levels: vec![level("xhigh", "Ultra")],
            default: None,
        });
        kind.model = Some(KindModel {
            choices: vec![choice("opus", "Opus", true, Some(&["xhigh", "turbo"]))],
        });
        let models = declared_models(&kind);
        let ids: Vec<&str> = models[0]
            .efforts
            .iter()
            .map(|effort| effort.id.as_str())
            .collect();
        assert_eq!(ids, ["xhigh", "turbo"]);
        assert_eq!(labels(&models[0]), ["Ultra", "turbo"]);
    }

    #[test]
    fn a_kind_with_no_declared_models_is_offered_as_itself() {
        let kind = agent_kind("codex", "Codex", PackageState::Enabled);
        assert!(declared_models(&kind).is_empty());
        let kinds = [kind];
        assert_eq!(
            choice_kind(&kinds, "codex").map(|kind| kind.id.as_str()),
            Some("codex")
        );
    }

    #[test]
    fn a_model_belongs_to_the_kind_that_declares_it() {
        let kinds = [claude()];
        let (kind, model) = declared_model(&kinds, "sonnet").expect("sonnet is declared");
        assert_eq!(
            (kind.id.as_str(), model.label.as_str()),
            ("claude", "Sonnet")
        );
        assert_eq!(
            choice_kind(&kinds, "sonnet").map(|kind| kind.id.as_str()),
            Some("claude")
        );
        assert!(declared_model(&kinds, "gone").is_none());
    }

    #[test]
    fn the_preselected_model_is_the_first_declared_default() {
        assert_eq!(preselected_model(&[claude()]).as_deref(), Some("sonnet"));
        let plain = agent_kind("codex", "Codex", PackageState::Enabled);
        assert_eq!(preselected_model(&[plain]), None);
    }
}
