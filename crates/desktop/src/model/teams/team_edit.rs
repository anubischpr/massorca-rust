//! What an edit of a team changes (the Edit team form, screen 4.5). A field the form leaves as it was is not
//! sent, so the service changes only the fields that were edited.

use massorca_protocol::methods::teams::TeamCharacter;

use super::state::{TeamError, check_name};
use super::types::Team;

/// The values the Edit team form holds when it is sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamForm {
    pub name: String,
    pub purpose: String,
    pub character: TeamCharacter,
    pub workspaces: bool,
}

/// The fields an edit changes; `None` leaves a field as it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TeamChange {
    pub name: Option<String>,
    pub purpose: Option<String>,
    pub character: Option<TeamCharacter>,
    pub workspaces: Option<bool>,
}

impl TeamForm {
    /// The form as it opens on `team`.
    pub fn of(team: &Team) -> Self {
        Self {
            name: team.name.clone(),
            purpose: team.purpose.clone(),
            character: team.character,
            workspaces: team.workspaces,
        }
    }
}

impl TeamChange {
    /// The fields of `form` that differ from the form as it `opened`, so a change another client made meanwhile is
    /// not sent back; the name is trimmed and checked, the purpose compared trimmed.
    pub fn between(opened: &TeamForm, form: &TeamForm) -> Result<Self, TeamError> {
        let name = check_name(&form.name)?;
        let purpose = form.purpose.trim();
        Ok(Self {
            name: (name != opened.name.trim()).then_some(name),
            purpose: (purpose != opened.purpose.trim()).then(|| purpose.to_owned()),
            character: (form.character != opened.character).then_some(form.character),
            workspaces: (form.workspaces != opened.workspaces).then_some(form.workspaces),
        })
    }

    /// Whether the form changed nothing, so nothing is sent.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The shapes an edit offers, in the order the form lists them, with the words it shows.
pub const CHARACTER_CHOICES: [(TeamCharacter, &str); 6] = [
    (TeamCharacter::Triangle, "Triangle"),
    (TeamCharacter::Square, "Square"),
    (TeamCharacter::Circle, "Circle"),
    (TeamCharacter::Diamond, "Diamond"),
    (TeamCharacter::Hexagon, "Hexagon"),
    (TeamCharacter::Star, "Star"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::teams::{SampleTeamsSource, TeamId, TeamsSource};

    fn desarrollo() -> Team {
        SampleTeamsSource
            .teams()
            .team(TeamId(1))
            .cloned()
            .expect("the sample team is there")
    }

    fn form_of(team: &Team) -> TeamForm {
        TeamForm::of(team)
    }

    #[test]
    fn a_rename_made_elsewhere_while_the_form_is_open_is_not_sent_back() {
        let team = desarrollo();
        let opened = form_of(&team);
        let form = TeamForm {
            workspaces: !team.workspaces,
            ..opened.clone()
        };
        assert_eq!(
            TeamChange::between(&opened, &form),
            Ok(TeamChange {
                workspaces: Some(!team.workspaces),
                ..TeamChange::default()
            })
        );
    }

    #[test]
    fn only_the_fields_the_form_changed_are_sent() {
        let team = desarrollo();
        let form = TeamForm {
            name: "Platform".to_owned(),
            ..form_of(&team)
        };
        assert_eq!(
            TeamChange::between(&form_of(&team), &form),
            Ok(TeamChange {
                name: Some("Platform".to_owned()),
                ..TeamChange::default()
            })
        );
    }

    #[test]
    fn an_unchanged_form_sends_nothing_and_spaces_around_the_name_are_not_a_change() {
        let team = desarrollo();
        let padded = TeamForm {
            name: format!("  {}  ", team.name),
            purpose: format!(" {} ", team.purpose),
            ..form_of(&team)
        };
        let change = TeamChange::between(&form_of(&team), &padded).expect("the form is valid");
        assert!(change.is_empty(), "{change:?}");
    }

    #[test]
    fn a_shape_and_the_workspaces_flag_are_sent_when_they_change() {
        let team = desarrollo();
        let form = TeamForm {
            character: TeamCharacter::Star,
            workspaces: !team.workspaces,
            ..form_of(&team)
        };
        assert_eq!(
            TeamChange::between(&form_of(&team), &form),
            Ok(TeamChange {
                character: Some(TeamCharacter::Star),
                workspaces: Some(!team.workspaces),
                ..TeamChange::default()
            })
        );
    }

    #[test]
    fn a_blank_name_is_refused_before_anything_is_sent() {
        let team = desarrollo();
        let form = TeamForm {
            name: "   ".to_owned(),
            ..form_of(&team)
        };
        assert_eq!(
            TeamChange::between(&form_of(&team), &form),
            Err(TeamError::NameRequired)
        );
    }
}
