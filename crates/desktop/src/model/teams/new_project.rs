//! The New project form (pv96): a name, a goal and, for a team with Workspaces, where the code is.

use massorca_protocol::methods::teams::CodeSource;

use super::state::{TeamError, check_name};

/// What the New project dialog holds when it is sent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewProject {
    pub name: String,
    pub goal: String,
    /// Where the code is, as the owner picked it.
    pub code_source: Option<CodeSource>,
}

/// A New project form that passed its checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectDraft {
    pub name: String,
    pub goal: Option<String>,
    pub code_source: Option<CodeSource>,
}

impl NewProject {
    /// Checks the form. The service refuses a code source in a team without Workspaces, so one is kept only when
    /// `workspaces` is on.
    pub fn check(&self, workspaces: bool) -> Result<ProjectDraft, TeamError> {
        let name = check_name(&self.name)?;
        let goal = self.goal.trim();
        Ok(ProjectDraft {
            name,
            goal: (!goal.is_empty()).then(|| goal.to_owned()),
            code_source: self.code_source.filter(|_| workspaces),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(name: &str, goal: &str) -> NewProject {
        NewProject {
            name: name.into(),
            goal: goal.into(),
            code_source: None,
        }
    }

    #[test]
    fn a_project_needs_a_name_and_keeps_it_trimmed() {
        assert_eq!(form("  ", "").check(false), Err(TeamError::NameRequired));
        assert_eq!(
            form(" Checkout ", "").check(false).map(|draft| draft.name),
            Ok("Checkout".to_owned())
        );
    }

    #[test]
    fn a_blank_goal_is_sent_as_none_and_a_written_one_trimmed() {
        assert_eq!(
            form("Checkout", "   ").check(false).map(|draft| draft.goal),
            Ok(None)
        );
        assert_eq!(
            form("Checkout", " Ship the cart ")
                .check(false)
                .map(|draft| draft.goal),
            Ok(Some("Ship the cart".to_owned()))
        );
    }

    #[test]
    fn a_code_source_is_kept_only_for_a_team_with_workspaces() {
        let picked = NewProject {
            code_source: Some(CodeSource::ExistingRepository),
            ..form("Checkout", "")
        };
        assert_eq!(picked.check(false).map(|draft| draft.code_source), Ok(None));
        assert_eq!(
            picked.check(true).map(|draft| draft.code_source),
            Ok(Some(CodeSource::ExistingRepository))
        );
    }
}
