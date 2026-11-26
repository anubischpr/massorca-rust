//! The rollback confirm's words (owner ruling 2026-11-25): one sentence on what a rollback puts back in MassOrca, and the
//! versions and the consent it accepts, under Show details.

/// What the rollback confirm says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackOutcome {
    pub title: String,
    pub sentence: String,
    pub button: &'static str,
}

/// The confirm for putting back `previous` of the package called `name`.
#[must_use]
pub fn rollback_outcome(name: &str, previous: &str) -> RollbackOutcome {
    RollbackOutcome {
        title: format!("Roll back {name}?"),
        sentence: format!(
            "MassOrca will put back {name} {previous}. Your current version is kept so you can switch back."
        ),
        button: "Roll back",
    }
}

/// The technical lines of a rollback, for Show details: the versions it moves between and the consent it accepts.
#[must_use]
pub fn rollback_details(from: &str, to: &str, consent: &str) -> Vec<String> {
    vec![
        format!("Version from {from} to {to}."),
        format!("Consent fingerprint: {consent}."),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_confirm_names_the_package_and_the_version_it_puts_back() {
        assert_eq!(
            rollback_outcome("Notes", "1.0.0"),
            RollbackOutcome {
                title: "Roll back Notes?".to_owned(),
                sentence: "MassOrca will put back Notes 1.0.0. Your current version is kept so you can switch back."
                    .to_owned(),
                button: "Roll back",
            }
        );
    }

    #[test]
    fn show_details_holds_the_versions_and_the_consent_it_accepts() {
        assert_eq!(
            rollback_details("2.0.0", "1.0.0", "fp-1"),
            vec![
                "Version from 2.0.0 to 1.0.0.".to_owned(),
                "Consent fingerprint: fp-1.".to_owned(),
            ]
        );
    }
}
