//! The install review dialog's words (decision 0062, owner ruling 2026-11-24): what the button will do, always shown.
//! The technical review is under Show details (`install_details.rs`).

use massorca_protocol::methods::extensions::{CliPlanState, ExtensionReview};

/// What the dialog says, in one of two cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewOutcome {
    /// Nothing is installed on this computer: the package is added to MassOrca.
    Add { title: String, body: String },
    /// The vendor installer runs: the sentence and the command on its own line are the warning.
    Install {
        title: String,
        sentence: String,
        command: String,
    },
}

impl ReviewOutcome {
    pub fn title(&self) -> &str {
        match self {
            Self::Add { title, .. } | Self::Install { title, .. } => title,
        }
    }

    /// The primary button.
    pub fn button(&self) -> &'static str {
        match self {
            Self::Add { .. } => "Add to MassOrca",
            Self::Install { .. } => "Install",
        }
    }
}

/// Whether this computer runs a vendor installer: a CLI plan that will run its command.
pub fn installer_runs(review: &ExtensionReview) -> bool {
    review
        .cli
        .iter()
        .any(|plan| matches!(plan.state, CliPlanState::WillRun { .. }))
}

/// The dialog's words for `review`, naming the item `name` (the catalogue item's display name).
pub fn review_outcome(review: &ExtensionReview, name: &str) -> ReviewOutcome {
    let commands: Vec<String> = review
        .cli
        .iter()
        .filter_map(|plan| match &plan.state {
            CliPlanState::WillRun { command, .. } => Some(command.join(" ")),
            _ => None,
        })
        .collect();
    if !commands.is_empty() {
        return ReviewOutcome::Install {
            title: format!("Install {name}"),
            sentence: format!("MassOrca will install {name} on this computer."),
            command: commands.join("; "),
        };
    }
    let detected = review
        .cli
        .iter()
        .any(|plan| matches!(plan.state, CliPlanState::Detected { .. }));
    let body = if detected {
        format!("MassOrca will add the {name} agent you already have installed on this computer.")
    } else {
        format!("MassOrca will add {name} to this computer.")
    };
    ReviewOutcome::Add {
        title: format!("Add {name} to MassOrca"),
        body,
    }
}

#[cfg(test)]
pub(super) mod tests {
    use massorca_protocol::methods::extensions::{
        CliPlan, CliPlanState, KindView, Listing, Permission, Risk, RiskLevel, SourceKind,
        SourceRef,
    };

    use super::*;

    pub(in crate::model::marketplace) fn review() -> ExtensionReview {
        ExtensionReview {
            id: "massorca.claude".to_owned(),
            name: "Claude".to_owned(),
            version: "1.0.0".to_owned(),
            publisher: "massorca".to_owned(),
            description: "Starts Claude sessions.".to_owned(),
            source: SourceRef {
                kind: SourceKind::Catalogue,
                reference: "massorca.claude".to_owned(),
            },
            sha256: "abc123".to_owned(),
            permissions: vec![Permission {
                reason: "Reads the project.".to_owned(),
                ..crate::service::fixtures::base_permission()
            }],
            risk: Risk {
                level: RiskLevel::High,
                reasons: Vec::new(),
            },
            listing: Listing {
                logo: None,
                category: "agents".to_owned(),
                developer: None,
                mention: None,
                banner: None,
                connections: Vec::new(),
                website: None,
                privacy: None,
                terms: None,
            },
            kinds: vec![KindView {
                program: vec!["claude".to_owned()],
                args: vec!["--json".to_owned()],
                env: vec!["CLAUDE_HOME".to_owned()],
                ..crate::service::fixtures::base_kind_view()
            }],
            mcp_servers: Vec::new(),
            cli: vec![CliPlan {
                agent: "claude".to_owned(),
                binary: Some("claude".to_owned()),
                verify: Some(vec!["claude".to_owned(), "--version".to_owned()]),
                uninstall: None,
                state: CliPlanState::WillRun {
                    command: vec![
                        "winget".to_owned(),
                        "install".to_owned(),
                        "Anthropic.ClaudeCode".to_owned(),
                    ],
                    runs_package_scripts: false,
                },
            }],
            runs_installer: true,
            consent: "fingerprint-1".to_owned(),
        }
    }

    /// The review of a CLI already on this computer: the installer is skipped.
    pub(in crate::model::marketplace) fn detected_review() -> ExtensionReview {
        let mut found = review();
        found.cli[0].state = CliPlanState::Detected { path: None };
        found.runs_installer = false;
        found
    }

    /// Text the dialog no longer shows in any case.
    const REMOVED: [&str; 10] = [
        "SHA-256",
        "Consent",
        "Permissions",
        "Runs a program",
        "Gives agents access",
        "starts:",
        "keeps its sign-in in",
        "Verify after the install",
        "Uninstall command",
        "Risk",
    ];

    fn words(outcome: &ReviewOutcome) -> String {
        match outcome {
            ReviewOutcome::Add { title, body } => format!(
                "{title}
{body}"
            ),
            ReviewOutcome::Install {
                title,
                sentence,
                command,
            } => format!(
                "{title}
{sentence}
{command}"
            ),
        }
    }

    fn assert_nothing_removed_shows(outcome: &ReviewOutcome) {
        let text = words(outcome);
        for removed in REMOVED {
            assert!(!text.contains(removed), "{removed:?} shows in {text:?}");
        }
    }

    #[test]
    fn a_cli_already_here_is_added_to_massorca_and_nothing_is_installed() {
        let outcome = review_outcome(&detected_review(), "Claude");
        assert_eq!(
            outcome,
            ReviewOutcome::Add {
                title: "Add Claude to MassOrca".to_owned(),
                body: "MassOrca will add the Claude agent you already have installed on this computer."
                    .to_owned(),
            }
        );
        assert_eq!(outcome.button(), "Add to MassOrca");
        assert_nothing_removed_shows(&outcome);
    }

    #[test]
    fn a_missing_cli_names_the_one_installer_command() {
        let outcome = review_outcome(&review(), "Claude");
        assert_eq!(
            outcome,
            ReviewOutcome::Install {
                title: "Install Claude".to_owned(),
                sentence: "MassOrca will install Claude on this computer.".to_owned(),
                command: "winget install Anthropic.ClaudeCode".to_owned(),
            }
        );
        assert_eq!(outcome.button(), "Install");
        assert_nothing_removed_shows(&outcome);
    }

    #[test]
    fn a_package_without_an_installer_is_added_by_name() {
        let mut plain = review();
        plain.cli.clear();
        plain.kinds.clear();
        plain.runs_installer = false;
        let outcome = review_outcome(&plain, "Kit");
        assert_eq!(
            outcome,
            ReviewOutcome::Add {
                title: "Add Kit to MassOrca".to_owned(),
                body: "MassOrca will add Kit to this computer.".to_owned(),
            }
        );
        assert_eq!(outcome.button(), "Add to MassOrca");
        assert_nothing_removed_shows(&outcome);
    }
}
