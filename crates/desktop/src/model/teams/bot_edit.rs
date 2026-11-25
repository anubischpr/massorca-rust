//! The Edit bot form: a bot's name, job and agent kind, and the fields an edit sends (`team.bot_update`).

use super::state::{TeamError, check_name};
use super::types::Bot;

/// The values the Edit bot form holds: the form as it opened, and as it is sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BotForm {
    pub name: String,
    pub job: String,
    /// The service's id of the agent kind the bot runs on.
    pub kind: String,
}

/// The fields an edit changes; `None` leaves a field as it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BotChange {
    pub name: Option<String>,
    pub job: Option<String>,
    pub kind: Option<String>,
}

impl BotForm {
    /// The form as it opens on `bot`.
    pub fn of(bot: &Bot) -> Self {
        Self {
            name: bot.name.clone(),
            job: bot.role.clone(),
            kind: bot.kind.clone(),
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
        })
    }

    /// Whether the form changed nothing, so nothing is sent.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened() -> BotForm {
        BotForm {
            name: "Nora".into(),
            job: "Reviews".into(),
            kind: "claude".into(),
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
                job: None,
                kind: None,
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
                name: None,
                job: None,
                kind: Some("codex".into()),
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
}
