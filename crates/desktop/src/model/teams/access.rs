//! The words a team shows for its access and the one-step change to it (decision 0072, amendment "team
//! access"). The card, the Board header and the New team dialog draw these, so the wording lives once.

use massorca_protocol::methods::teams::TeamAccess;

/// What a full-access team shows: its bots run without asking (decision 0083 names the bypass mode).
const FULL_ACCESS: &str = "Full access: bots run without asking for permission";

/// The line a team shows for its access.
pub fn access_line(access: TeamAccess) -> &'static str {
    match access {
        TeamAccess::Full => FULL_ACCESS,
        TeamAccess::Default => "Default access: bots may ask for permission",
        TeamAccess::Unknown => "Access: not known to this version",
    }
}

/// The one-step change a team offers, with the access it sets. A value this build does not know offers none.
pub fn access_change(access: TeamAccess) -> Option<(&'static str, TeamAccess)> {
    match access {
        TeamAccess::Full => Some(("Ask for permission", TeamAccess::Default)),
        TeamAccess::Default => Some(("Run without asking", TeamAccess::Full)),
        TeamAccess::Unknown => None,
    }
}

/// The New team dialog's sentence: the access a new team gets, and where to change it.
pub fn new_team_access_hint() -> String {
    format!("{FULL_ACCESS}. Change it later on the team card.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_team_says_its_bots_run_without_asking_and_offers_to_ask() {
        assert!(
            access_line(TeamAccess::Full)
                .contains("Full access: bots run without asking for permission")
        );
        assert_eq!(
            access_change(TeamAccess::Full),
            Some(("Ask for permission", TeamAccess::Default))
        );
    }

    #[test]
    fn a_default_team_offers_to_run_without_asking_and_an_unknown_one_offers_nothing() {
        assert_eq!(
            access_change(TeamAccess::Default),
            Some(("Run without asking", TeamAccess::Full))
        );
        assert_eq!(access_change(TeamAccess::Unknown), None);
    }

    #[test]
    fn the_new_team_sentence_names_full_access_and_where_to_change_it() {
        let hint = new_team_access_hint();
        assert!(hint.starts_with("Full access: bots run without asking for permission"));
        assert!(hint.contains("team card"));
    }
}
