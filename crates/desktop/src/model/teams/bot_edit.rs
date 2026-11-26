//! The Edit bot form: a bot's name, job, agent kind and Advanced options, and the fields an edit sends (`team.bot_update`).

use super::state::{TeamError, check_name};
use super::types::Bot;

/// The values the Edit bot form holds: the form as it opened, and as it is sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BotForm {
    pub name: String,
    pub job: String,
    /// The service's id of the agent kind the bot runs on.
    pub kind: String,
    /// The model the bot runs on; `None` leaves it to the bot's job.
    pub model: Option<String>,
    /// The effort the bot runs at; `None` leaves it to the bot's job.
    pub effort: Option<String>,
}

/// The fields an edit changes; `None` leaves a field as it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BotChange {
    pub name: Option<String>,
    pub job: Option<String>,
    pub kind: Option<String>,
    /// `Some("")` clears the model, so the bot's job picks it again.
    pub model: Option<String>,
    /// `Some("")` clears the effort, so the bot's job picks it again.
    pub effort: Option<String>,
}

impl BotForm {
    /// The form as it opens on `bot`.
    pub fn of(bot: &Bot) -> Self {
        Self {
            name: bot.name.clone(),
            job: bot.role.clone(),
            kind: bot.kind.clone(),
            model: bot.model.clone(),
            effort: bot.effort.clone(),
        }
    }
}

impl BotChange {
    /// The fields of `form` that differ from the form as it `opened`, so a change another client made meanwhile is
    /// not sent back. The name is trimmed and checked, the job compared trimmed.
    pub fn between(opened: &BotForm, form: &BotForm) -> Result<Self, TeamError> {
        let name = check_name(&form.name)?;
        let job = form.job.trim();
        Ok(Self {
            name: (name != opened.name.trim()).then_some(name),
            job: (job != opened.job.trim()).then(|| job.to_owned()),
            kind: (form.kind != opened.kind).then(|| form.kind.clone()),
            model: changed_option(&opened.model, &form.model),
            effort: changed_option(&opened.effort, &form.effort),
        })
    }

    /// Whether the form changed nothing, so nothing is sent.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The value an optional field sends when it changed: its value, or empty to clear it when it was set to nothing.
fn changed_option(opened: &Option<String>, form: &Option<String>) -> Option<String> {
    (form != opened).then(|| form.clone().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::super::types::BotId;
    use super::*;

    fn opened() -> BotForm {
        BotForm {
            name: "Nora".into(),
            job: "Reviews".into(),
            kind: "claude".into(),
            model: None,
            effort: None,
        }
    }

    fn stored() -> BotForm {
        BotForm {
            model: Some("opus".into()),
            effort: Some("high".into()),
            ..opened()
        }
    }

    #[test]
    fn a_form_saved_unchanged_sends_nothing() {
        let change = BotChange::between(&opened(), &opened()).expect("the form is valid");
        assert!(change.is_empty());
    }

    #[test]
    fn only_the_field_that_changed_is_sent() {
        let form = BotForm {
            name: " Lucia ".into(),
            ..opened()
        };
        assert_eq!(
            BotChange::between(&opened(), &form),
            Ok(BotChange {
                name: Some("Lucia".into()),
                ..BotChange::default()
            })
        );
    }

    #[test]
    fn the_job_is_compared_trimmed_and_a_new_kind_is_sent() {
        let form = BotForm {
            job: "Reviews  ".into(),
            kind: "codex".into(),
            ..opened()
        };
        assert_eq!(
            BotChange::between(&opened(), &form),
            Ok(BotChange {
                kind: Some("codex".into()),
                ..BotChange::default()
            })
        );
    }

    #[test]
    fn a_blank_name_is_refused() {
        let form = BotForm {
            name: "  ".into(),
            ..opened()
        };
        assert_eq!(
            BotChange::between(&opened(), &form),
            Err(TeamError::NameRequired)
        );
    }

    #[test]
    fn a_model_and_effort_that_are_set_are_sent() {
        let form = BotForm {
            model: Some("opus".into()),
            effort: Some("high".into()),
            ..opened()
        };
        assert_eq!(
            BotChange::between(&opened(), &form),
            Ok(BotChange {
                model: Some("opus".into()),
                effort: Some("high".into()),
                ..BotChange::default()
            })
        );
    }

    #[test]
    fn a_model_and_effort_set_back_to_the_job_are_sent_empty_to_clear_them() {
        let form = BotForm {
            model: None,
            effort: None,
            ..stored()
        };
        assert_eq!(
            BotChange::between(&stored(), &form),
            Ok(BotChange {
                model: Some(String::new()),
                effort: Some(String::new()),
                ..BotChange::default()
            })
        );
    }

    #[test]
    fn stored_advanced_options_left_as_they_were_send_nothing() {
        let form = BotForm {
            name: "Lucia".into(),
            ..stored()
        };
        assert_eq!(
            BotChange::between(&stored(), &form),
            Ok(BotChange {
                name: Some("Lucia".into()),
                ..BotChange::default()
            })
        );
    }

    #[test]
    fn a_form_opens_with_the_bots_stored_model_and_effort() {
        let bot = Bot {
            id: BotId(1),
            name: "Nora".into(),
            role: "Reviews".into(),
            kind: "claude".into(),
            model: Some("opus".into()),
            effort: Some("high".into()),
            agent: None,
            kind_unavailable: None,
        };
        assert_eq!(BotForm::of(&bot), stored());
    }
}
