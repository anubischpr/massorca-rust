//! The rows of a team card's menu (screen 4.5): Edit team, the one-step access change, then Delete team.

use massorca_protocol::methods::teams::TeamAccess;

use super::access::access_change;
use super::types::Team;

/// One row of a team card's menu, as plain words for the shell to draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardMenuRow {
    Edit,
    /// The one-step access change; `access` is the access the row sets.
    Access {
        label: &'static str,
        access: TeamAccess,
    },
    Separator,
    Delete,
}

/// The rows of a team's card menu, top to bottom. A team whose access this build does not know offers no access row.
pub fn card_menu_rows(team: &Team) -> Vec<CardMenuRow> {
    let mut rows = vec![CardMenuRow::Edit];
    if let Some((label, access)) = access_change(team.access) {
        rows.push(CardMenuRow::Access { label, access });
    }
    rows.push(CardMenuRow::Separator);
    rows.push(CardMenuRow::Delete);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::teams::{SampleTeamsSource, TeamId, TeamsSource};

    fn team_with(access: TeamAccess) -> Team {
        let mut team = SampleTeamsSource
            .teams()
            .team(TeamId(1))
            .cloned()
            .expect("the sample team is there");
        team.access = access;
        team
    }

    #[test]
    fn a_full_team_offers_default_access_between_edit_and_a_separated_delete() {
        assert_eq!(
            card_menu_rows(&team_with(TeamAccess::Full)),
            vec![
                CardMenuRow::Edit,
                CardMenuRow::Access {
                    label: "Ask for permission",
                    access: TeamAccess::Default,
                },
                CardMenuRow::Separator,
                CardMenuRow::Delete,
            ]
        );
    }

    #[test]
    fn a_default_team_offers_full_access_and_an_unknown_one_offers_no_access_row() {
        assert_eq!(
            card_menu_rows(&team_with(TeamAccess::Default))[1],
            CardMenuRow::Access {
                label: "Run without asking",
                access: TeamAccess::Full,
            }
        );
        assert_eq!(
            card_menu_rows(&team_with(TeamAccess::Unknown)),
            vec![
                CardMenuRow::Edit,
                CardMenuRow::Separator,
                CardMenuRow::Delete
            ]
        );
    }
}
