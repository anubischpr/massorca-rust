//! The Model and Effort choices a bot form offers under Advanced options, read from `agent.kinds` (decision 0096).
//! A bot leaves both to its job unless the person sets them, so each list starts with "Picked from the job".

use massorca_protocol::methods::agents::AgentKind;

/// The row that leaves a value to the bot's job; the service picks it.
pub const PICKED_FROM_JOB: &str = "Picked from the job";

/// The models and effort levels one agent kind offers, in the order the service lists them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KindOptions {
    /// The service's id of the kind.
    pub kind: String,
    /// Empty when the kind's list is account-scoped, which the vendor CLI reports.
    pub models: Vec<ModelOption>,
    pub levels: Vec<EffortOption>,
}

/// A model a kind offers. `efforts` lists the levels it accepts by id: `None` accepts every level of the kind,
/// and an empty list takes none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
    pub efforts: Option<Vec<String>>,
}

/// An effort level a kind offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffortOption {
    pub id: String,
    pub label: String,
}

/// One row of a Model or Effort select. `value` is what the bot stores, `None` for "Picked from the job".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionRow {
    pub value: Option<String>,
    pub label: String,
    /// `false` for a stored value the kind no longer offers: the row shows it, but it cannot be chosen.
    pub available: bool,
}

/// Every kind's choices, in the order the service lists the kinds.
pub fn kind_options(kinds: &[AgentKind]) -> Vec<KindOptions> {
    kinds
        .iter()
        .map(|kind| KindOptions {
            kind: kind.id.clone(),
            models: kind.model.as_ref().map_or_else(Vec::new, |model| {
                model
                    .choices
                    .iter()
                    .map(|choice| ModelOption {
                        id: choice.id.clone(),
                        label: choice.label.clone(),
                        efforts: choice.efforts.clone(),
                    })
                    .collect()
            }),
            levels: kind.effort.as_ref().map_or_else(Vec::new, |effort| {
                effort
                    .levels
                    .iter()
                    .map(|level| EffortOption {
                        id: level.id.clone(),
                        label: level.label.clone(),
                    })
                    .collect()
            }),
        })
        .collect()
}

/// The options of `kind`, if the service lists it.
pub fn options_of<'a>(catalogue: &'a [KindOptions], kind: &str) -> Option<&'a KindOptions> {
    catalogue.iter().find(|options| options.kind == kind)
}

/// The rows of the Model select for `kind`. A `stored` model the kind does not list stays as an unavailable row,
/// so saving keeps it until the person chooses another.
pub fn model_rows(kind: Option<&KindOptions>, stored: Option<&str>) -> Vec<OptionRow> {
    let models: &[ModelOption] = kind.map_or(&[], |kind| kind.models.as_slice());
    let mut rows = vec![picked_from_job()];
    rows.extend(models.iter().map(|model| OptionRow {
        value: Some(model.id.clone()),
        label: model.label.clone(),
        available: true,
    }));
    if let Some(stored) = stored
        && !models.iter().any(|model| model.id == stored)
    {
        rows.push(unavailable(stored));
    }
    rows
}

/// The rows of the Effort select for `kind` with `model` chosen. Without a model, every level of the kind is
/// offered; a model that takes no effort leaves only "Picked from the job". A `stored` level the model does not
/// accept stays as an unavailable row.
pub fn effort_rows(
    kind: Option<&KindOptions>,
    model: Option<&str>,
    stored: Option<&str>,
) -> Vec<OptionRow> {
    let levels = offered_levels(kind, model);
    let mut rows = vec![picked_from_job()];
    rows.extend(levels.iter().map(|level| OptionRow {
        value: Some(level.id.clone()),
        label: level.label.clone(),
        available: true,
    }));
    if let Some(stored) = stored
        && !levels.iter().any(|level| level.id == stored)
    {
        rows.push(unavailable(stored));
    }
    rows
}

/// The index of the row that holds `value`, or the "Picked from the job" row when none does.
pub fn row_index(rows: &[OptionRow], value: Option<&str>) -> usize {
    rows.iter()
        .position(|row| row.value.as_deref() == value)
        .unwrap_or(0)
}

/// The effort levels `model` accepts, in the kind's order.
fn offered_levels<'a>(kind: Option<&'a KindOptions>, model: Option<&str>) -> Vec<&'a EffortOption> {
    let Some(kind) = kind else {
        return Vec::new();
    };
    let accepted: Option<&[String]> = model
        .and_then(|id| kind.models.iter().find(|candidate| candidate.id == id))
        .and_then(|candidate| candidate.efforts.as_deref());
    kind.levels
        .iter()
        .filter(|level| accepted.is_none_or(|ids| ids.contains(&level.id)))
        .collect()
}

fn picked_from_job() -> OptionRow {
    OptionRow {
        value: None,
        label: PICKED_FROM_JOB.to_owned(),
        available: true,
    }
}

fn unavailable(stored: &str) -> OptionRow {
    OptionRow {
        value: Some(stored.to_owned()),
        label: stored.to_owned(),
        available: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::agent_kind;
    use massorca_protocol::methods::agents::{
        KindEffort, KindEffortLevel, KindModel, KindModelChoice, PackageState,
    };

    fn choice(id: &str, label: &str, efforts: Option<&[&str]>) -> KindModelChoice {
        KindModelChoice {
            id: id.into(),
            label: label.into(),
            default: false,
            efforts: efforts.map(|ids| ids.iter().map(|id| (*id).to_owned()).collect()),
            fast: false,
        }
    }

    fn level(id: &str, label: &str) -> KindEffortLevel {
        KindEffortLevel {
            id: id.into(),
            label: label.into(),
        }
    }

    /// Claude as `agent.kinds` lists it: Haiku takes only low effort, and the kind declares two levels.
    fn claude() -> KindOptions {
        let kind = AgentKind {
            model: Some(KindModel {
                choices: vec![
                    choice("opus", "Opus", None),
                    choice("sonnet", "Sonnet", None),
                    choice("haiku", "Haiku", Some(&["low"])),
                ],
            }),
            effort: Some(KindEffort {
                levels: vec![level("low", "Low"), level("high", "High")],
                default: Some("high".into()),
            }),
            ..agent_kind("claude", "Claude", PackageState::Enabled)
        };
        kind_options(&[kind])
            .into_iter()
            .next()
            .expect("the kind is listed")
    }

    fn labels(rows: &[OptionRow]) -> Vec<&str> {
        rows.iter().map(|row| row.label.as_str()).collect()
    }

    #[test]
    fn the_job_row_leads_the_models_in_the_kinds_order() {
        assert_eq!(
            labels(&model_rows(Some(&claude()), None)),
            ["Picked from the job", "Opus", "Sonnet", "Haiku"]
        );
    }

    #[test]
    fn a_kind_that_declares_no_models_offers_only_the_job_row() {
        assert_eq!(labels(&model_rows(None, None)), ["Picked from the job"]);
    }

    #[test]
    fn a_stored_model_the_kind_no_longer_lists_stays_as_an_unavailable_row() {
        let rows = model_rows(Some(&claude()), Some("gpt-old"));
        assert_eq!(
            rows.last(),
            Some(&OptionRow {
                value: Some("gpt-old".into()),
                label: "gpt-old".into(),
                available: false,
            })
        );
    }

    #[test]
    fn a_model_that_lists_its_efforts_offers_only_those_levels() {
        assert_eq!(
            labels(&effort_rows(Some(&claude()), Some("haiku"), None)),
            ["Picked from the job", "Low"]
        );
    }

    #[test]
    fn with_no_model_chosen_every_level_of_the_kind_is_offered() {
        assert_eq!(
            labels(&effort_rows(Some(&claude()), None, None)),
            ["Picked from the job", "Low", "High"]
        );
    }

    #[test]
    fn a_model_without_its_own_list_accepts_every_level() {
        assert_eq!(
            labels(&effort_rows(Some(&claude()), Some("opus"), None)),
            ["Picked from the job", "Low", "High"]
        );
    }

    #[test]
    fn a_stored_level_the_model_does_not_accept_stays_unavailable() {
        let rows = effort_rows(Some(&claude()), Some("haiku"), Some("high"));
        assert_eq!(
            rows.last(),
            Some(&OptionRow {
                value: Some("high".into()),
                label: "high".into(),
                available: false,
            })
        );
    }

    #[test]
    fn a_declared_default_never_preselects_a_model() {
        let mut kind = agent_kind("claude", "Claude", PackageState::Enabled);
        let mut opus = choice("opus", "Opus", None);
        opus.default = true;
        kind.model = Some(KindModel {
            choices: vec![opus],
        });
        let options = kind_options(&[kind]);
        let rows = model_rows(options.first(), None);
        assert_eq!(row_index(&rows, None), 0);
    }

    #[test]
    fn a_value_no_row_holds_falls_back_to_the_job_row() {
        let rows = model_rows(Some(&claude()), None);
        assert_eq!(row_index(&rows, Some("sonnet")), 2);
        assert_eq!(row_index(&rows, Some("nothing")), 0);
    }
}
